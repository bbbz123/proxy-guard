from __future__ import annotations

import json
import os
import sys
import traceback
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
import tkinter as tk
from tkinter import messagebox, ttk
import tkinter.font as tkfont
import winreg


INTERNET_SETTINGS_SUBKEY = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings"
RUN_SUBKEY = r"Software\Microsoft\Windows\CurrentVersion\Run"
RUN_VALUE_NAME = "ProxyGuardHelper"
CONFIG_DIR_NAME = "ProxyGuard"
CONFIG_FILE_NAME = "config.json"
PORTABLE_MARKER_FILE = "proxy_guard.portable"
PORTABLE_CONFIG_DIR_NAME = "config"
HELPER_EXE_NAME = "proxy_guard_helper.exe"

WINDOW_WIDTH = 960
WINDOW_HEIGHT = 780
WINDOW_MIN_WIDTH = 860
WINDOW_MIN_HEIGHT = 680

COLORS = {
    "page": "#eef2f6",
    "card": "#ffffff",
    "card_alt": "#f8fbff",
    "border": "#d7dee8",
    "text": "#1f2937",
    "muted": "#5f6c7b",
    "accent": "#2f6fed",
    "accent_soft": "#e8f0ff",
    "success": "#2d8a4f",
    "success_soft": "#edf8f0",
    "button": "#f4f7fb",
}

CLEANUP_SCOPE_OPTIONS = [
    ("ShutdownAndRestart", "关机/重启（非注销，推荐）"),
    ("ShutdownRestartAndLogoff", "关机 + 重启 + 注销"),
]


@dataclass(frozen=True)
class ManualProxyEntry:
    protocol: str | None
    host: str
    port: int

    def sort_key(self) -> tuple[str, str, int]:
        return (self.protocol or "", self.host, self.port)


@dataclass(frozen=True)
class ManualProxySpec:
    has_protocol_map: bool
    entries: list[ManualProxyEntry]

    def normalized(self) -> str:
        entries = sorted(self.entries, key=lambda item: item.sort_key())
        if self.has_protocol_map:
            return ";".join(
                f"{entry.protocol or 'all'}={entry.host}:{entry.port}" for entry in entries
            )
        entry = entries[0]
        return f"{entry.host}:{entry.port}"

    def is_loopback_only(self) -> bool:
        return bool(self.entries) and all(is_loopback_host(entry.host) for entry in self.entries)


@dataclass(frozen=True)
class Candidate:
    candidate_id: str
    title: str
    detail: str
    recommended: bool
    rule: dict[str, Any]


class ProxyGuardSetupApp:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("Proxy Guard 设置")
        self.root.geometry(f"{WINDOW_WIDTH}x{WINDOW_HEIGHT}")
        self.root.minsize(WINDOW_MIN_WIDTH, WINDOW_MIN_HEIGHT)
        self.root.configure(bg=COLORS["page"])
        self.root.resizable(True, True)

        self.font_title = tkfont.Font(family="Segoe UI", size=18, weight="bold")
        self.font_section = tkfont.Font(family="Segoe UI", size=12, weight="bold")
        self.font_body = tkfont.Font(family="Segoe UI", size=10)
        self.font_body_bold = tkfont.Font(family="Segoe UI", size=10, weight="bold")
        self.font_meta = tkfont.Font(family="Segoe UI", size=9)
        self.font_badge = tkfont.Font(family="Segoe UI", size=9, weight="bold")
        self._wrap_widgets: list[tuple[tk.Widget, int, int]] = []
        self._candidate_detail_labels: list[tk.Label] = []

        self.config_data = load_config()
        self.candidates: list[Candidate] = []
        self.candidate_vars: dict[str, tk.BooleanVar] = {}

        normalized_scope = normalize_cleanup_scope(self.config_data["cleanup_scope"])
        self.scope_value = tk.StringVar(value=normalized_scope)
        self.scope_display = tk.StringVar(value=scope_value_to_label(normalized_scope))
        self.cleanup_on_login_var = tk.BooleanVar(
            value=bool(self.config_data.get("cleanup_on_login", False))
        )
        self.auto_start_helper_var = tk.BooleanVar(value=is_helper_auto_start_enabled())
        self.status_var = tk.StringVar(value=initial_loading_status_text())

        self._configure_styles()
        self._build_ui()
        self.root.bind("<Configure>", self._on_root_configure, add="+")
        self.root.after(0, self.scan_candidates)

    def _configure_styles(self) -> None:
        style = ttk.Style(self.root)
        for theme_name in ("vista", "xpnative", "clam"):
            if theme_name in style.theme_names():
                style.theme_use(theme_name)
                break

        style.configure("Card.TCheckbutton", font=self.font_body)
        style.configure("Card.TCombobox", padding=4)
        style.configure("Action.TButton", padding=(12, 8))

    def _build_ui(self) -> None:
        outer = tk.Frame(self.root, bg=COLORS["page"])
        outer.pack(fill="both", expand=True, padx=18, pady=18)
        outer.grid_columnconfigure(0, weight=1)
        outer.grid_rowconfigure(1, weight=1)

        header_card = self._create_card(outer, row=0, pady=(0, 14))
        header_card.grid_columnconfigure(1, weight=1)

        accent = tk.Frame(header_card, bg=COLORS["accent"], width=8)
        accent.grid(row=0, column=0, sticky="ns", padx=(0, 14))

        header_text = tk.Frame(header_card, bg=COLORS["card"])
        header_text.grid(row=0, column=1, sticky="nsew", pady=16)

        title_label = tk.Label(
            header_text,
            text="Proxy Guard",
            bg=COLORS["card"],
            fg=COLORS["text"],
            font=self.font_title,
            anchor="w",
        )
        title_label.pack(fill="x")

        subtitle_label = tk.Label(
            header_text,
            text="只管理你选中的系统代理项，在关机或重启时帮你清理残留，不碰 TUN、DNS、网卡和 WinHTTP。",
            bg=COLORS["card"],
            fg=COLORS["muted"],
            font=self.font_body,
            anchor="w",
            justify="left",
            wraplength=840,
        )
        subtitle_label.pack(fill="x", pady=(6, 0))
        self._register_wrap_widget(subtitle_label, horizontal_padding=24, minimum=540)

        body = tk.Frame(outer, bg=COLORS["page"])
        body.grid(row=1, column=0, sticky="nsew")
        body.grid_columnconfigure(0, weight=1)
        body.grid_rowconfigure(0, weight=1)

        self._build_candidates_section(body)
        self._build_scope_section(body)
        self._build_startup_section(body)
        self._build_status_section(body)
        self._build_actions_section(outer)

    def _create_card(
        self,
        parent: tk.Misc,
        row: int | None = None,
        column: int = 0,
        pady: tuple[int, int] | int = 0,
    ) -> tk.Frame:
        frame = tk.Frame(
            parent,
            bg=COLORS["card"],
            highlightbackground=COLORS["border"],
            highlightthickness=1,
            bd=0,
        )
        if row is None:
            frame.pack(fill="x")
        else:
            frame.grid(row=row, column=column, sticky="nsew", pady=pady)
        return frame

    def _build_section_header(
        self,
        parent: tk.Misc,
        title: str,
        subtitle: str,
    ) -> None:
        title_label = tk.Label(
            parent,
            text=title,
            bg=COLORS["card"],
            fg=COLORS["text"],
            font=self.font_section,
            anchor="w",
        )
        title_label.pack(fill="x", padx=16, pady=(14, 4))

        subtitle_label = tk.Label(
            parent,
            text=subtitle,
            bg=COLORS["card"],
            fg=COLORS["muted"],
            font=self.font_body,
            anchor="w",
            justify="left",
            wraplength=840,
        )
        subtitle_label.pack(fill="x", padx=16, pady=(0, 12))
        self._register_wrap_widget(subtitle_label, horizontal_padding=36, minimum=520)

    def _build_candidates_section(self, parent: tk.Frame) -> None:
        card = self._create_card(parent, row=0, pady=(0, 14))

        self._build_section_header(
            card,
            "托管代理",
            "只会清理你在这里勾选的代理项。优先勾选 127.0.0.1 / localhost / ::1 这类回环代理通常更稳。",
        )

        list_shell = tk.Frame(card, bg=COLORS["card"])
        list_shell.pack(fill="both", expand=True, padx=16, pady=(0, 16))
        list_shell.grid_columnconfigure(0, weight=1)
        list_shell.grid_rowconfigure(0, weight=1)

        canvas = tk.Canvas(
            list_shell,
            bg=COLORS["card"],
            highlightthickness=0,
            borderwidth=0,
        )
        scrollbar = ttk.Scrollbar(list_shell, orient="vertical", command=canvas.yview)
        canvas.configure(yscrollcommand=scrollbar.set)
        canvas.grid(row=0, column=0, sticky="nsew")
        scrollbar.grid(row=0, column=1, sticky="ns", padx=(10, 0))

        self.candidates_frame = tk.Frame(canvas, bg=COLORS["card"])
        self.candidates_window = canvas.create_window(
            (0, 0), window=self.candidates_frame, anchor="nw"
        )

        self.candidates_frame.bind(
            "<Configure>",
            lambda _event: canvas.configure(scrollregion=canvas.bbox("all")),
        )
        canvas.bind(
            "<Configure>",
            lambda event: canvas.itemconfigure(self.candidates_window, width=event.width),
        )
        canvas.bind_all("<MouseWheel>", self._on_mouse_wheel, add="+")
        self.candidates_canvas = canvas

    def _build_scope_section(self, parent: tk.Frame) -> None:
        card = self._create_card(parent, row=1, pady=(0, 14))

        self._build_section_header(
            card,
            "结束事件范围",
            "Windows 无法可靠区分“关机”和“重启”，所以这里统一使用“关机/重启（非注销）”。",
        )

        row = tk.Frame(card, bg=COLORS["card"])
        row.pack(fill="x", padx=16, pady=(0, 16))
        row.grid_columnconfigure(1, weight=1)

        tk.Label(
            row,
            text="清理时机",
            bg=COLORS["card"],
            fg=COLORS["text"],
            font=self.font_body_bold,
        ).grid(row=0, column=0, sticky="w")

        combo = ttk.Combobox(
            row,
            state="readonly",
            values=[label for _, label in CLEANUP_SCOPE_OPTIONS],
            textvariable=self.scope_display,
            style="Card.TCombobox",
        )
        combo.grid(row=0, column=1, sticky="ew", padx=(16, 0))
        combo.bind("<<ComboboxSelected>>", self._on_scope_changed)

    def _build_startup_section(self, parent: tk.Frame) -> None:
        card = self._create_card(parent, row=2, pady=(0, 14))

        self._build_section_header(
            card,
            "启动选项",
            "这部分只影响当前用户环境，不会改系统级网络组件。",
        )

        check_wrap = tk.Frame(card, bg=COLORS["card"])
        check_wrap.pack(fill="x", padx=16, pady=(0, 16))

        ttk.Checkbutton(
            check_wrap,
            text="登录后检查并清理残留代理",
            variable=self.cleanup_on_login_var,
            style="Card.TCheckbutton",
        ).pack(anchor="w", pady=(0, 10))

        ttk.Checkbutton(
            check_wrap,
            text="开机自动启动 Proxy Guard helper",
            variable=self.auto_start_helper_var,
            style="Card.TCheckbutton",
        ).pack(anchor="w")

    def _build_status_section(self, parent: tk.Frame) -> None:
        card = self._create_card(parent, row=3)

        self._build_section_header(
            card,
            "当前状态",
            "这里会显示扫描结果、保存结果和当前运行模式。",
        )

        status_box = tk.Frame(card, bg=COLORS["card_alt"])
        status_box.pack(fill="x", padx=16, pady=(0, 16))

        self.status_label = tk.Label(
            status_box,
            textvariable=self.status_var,
            bg=COLORS["card_alt"],
            fg=COLORS["muted"],
            font=self.font_body,
            justify="left",
            anchor="w",
            wraplength=820,
            padx=14,
            pady=12,
        )
        self.status_label.pack(fill="x")
        self._register_wrap_widget(self.status_label, horizontal_padding=40, minimum=500)

    def _build_actions_section(self, parent: tk.Frame) -> None:
        actions = tk.Frame(parent, bg=COLORS["page"])
        actions.grid(row=2, column=0, sticky="ew", pady=(14, 0))
        actions.grid_columnconfigure(1, weight=1)

        ttk.Button(
            actions,
            text="重新扫描",
            command=self.scan_candidates,
            style="Action.TButton",
        ).grid(row=0, column=0, sticky="w")

        ttk.Button(
            actions,
            text="保存设置",
            command=self.save_config_to_disk,
            style="Action.TButton",
        ).grid(row=0, column=2, sticky="e")

    def _on_mouse_wheel(self, event: tk.Event[tk.Misc]) -> None:
        if not hasattr(self, "candidates_canvas"):
            return
        widget = self.root.winfo_containing(event.x_root, event.y_root)
        if widget is None:
            return
        if str(widget).startswith(str(self.candidates_canvas)) or str(widget).startswith(
            str(self.candidates_frame)
        ):
            delta = -1 * int(event.delta / 120) if event.delta else 0
            if delta != 0:
                self.candidates_canvas.yview_scroll(delta, "units")

    def _on_root_configure(self, event: tk.Event[tk.Misc]) -> None:
        if event.widget is self.root:
            self._refresh_wrap_lengths()

    def _register_wrap_widget(
        self,
        widget: tk.Widget,
        *,
        horizontal_padding: int,
        minimum: int,
    ) -> None:
        self._wrap_widgets.append((widget, horizontal_padding, minimum))

    def _refresh_wrap_lengths(self) -> None:
        for widget, horizontal_padding, minimum in self._wrap_widgets:
            parent_width = widget.master.winfo_width() if widget.master is not None else 0
            width = max(minimum, parent_width - horizontal_padding)
            if width > 0:
                widget.configure(wraplength=width)

        candidate_area_width = self.candidates_frame.winfo_width()
        detail_wrap = max(420, candidate_area_width - 180)
        for label in self._candidate_detail_labels:
            label.configure(wraplength=detail_wrap)

    def _on_scope_changed(self, _event: tk.Event[tk.Misc]) -> None:
        selected_label = self.scope_display.get()
        for value, label in CLEANUP_SCOPE_OPTIONS:
            if label == selected_label:
                self.scope_value.set(value)
                return

    def scan_candidates(self) -> None:
        try:
            snapshot = load_system_proxy_snapshot()
            self.candidates = scan_candidates_from_snapshot(snapshot)
            self._render_candidates()
            self.status_var.set(initial_status_text(self.candidates))
        except Exception as exc:  # noqa: BLE001
            self.status_var.set(f"读取当前系统代理失败：{exc}")
            messagebox.showerror("Proxy Guard", f"读取当前系统代理失败：\n{exc}")

    def _render_candidates(self) -> None:
        for child in self.candidates_frame.winfo_children():
            child.destroy()

        self.candidate_vars.clear()
        self._candidate_detail_labels.clear()
        selected_rules = self.config_data.get("managed_rules", [])

        if not self.candidates:
            empty_box = tk.Frame(
                self.candidates_frame,
                bg=COLORS["card_alt"],
                highlightbackground=COLORS["border"],
                highlightthickness=1,
            )
            empty_box.pack(fill="x", pady=4)
            tk.Label(
                empty_box,
                text="当前没有检测到可托管的系统代理候选项。",
                bg=COLORS["card_alt"],
                fg=COLORS["muted"],
                font=self.font_body,
                padx=14,
                pady=14,
                anchor="w",
                justify="left",
            ).pack(fill="x")
            return

        for candidate in self.candidates:
            selected = is_rule_selected(candidate.rule, selected_rules)
            var = tk.BooleanVar(value=selected)
            self.candidate_vars[candidate.candidate_id] = var

            row_bg = COLORS["success_soft"] if candidate.recommended else COLORS["card_alt"]
            border_color = "#dce7f6" if not candidate.recommended else "#cfe5d4"
            accent_color = COLORS["success"] if candidate.recommended else COLORS["accent"]

            row = tk.Frame(
                self.candidates_frame,
                bg=row_bg,
                highlightbackground=border_color,
                highlightthickness=1,
            )
            row.pack(fill="x", pady=4)
            row.grid_columnconfigure(2, weight=1)

            accent = tk.Frame(row, bg=accent_color, width=6)
            accent.grid(row=0, column=0, rowspan=2, sticky="ns")

            check = ttk.Checkbutton(row, variable=var, style="Card.TCheckbutton")
            check.grid(row=0, column=1, rowspan=2, sticky="nw", padx=(10, 8), pady=12)

            title_wrap = tk.Frame(row, bg=row_bg)
            title_wrap.grid(row=0, column=2, sticky="ew", padx=(0, 12), pady=(12, 4))
            title_wrap.grid_columnconfigure(0, weight=1)

            tk.Label(
                title_wrap,
                text=candidate.title,
                bg=row_bg,
                fg=COLORS["text"],
                font=self.font_body_bold,
                anchor="w",
            ).grid(row=0, column=0, sticky="w")

            if candidate.recommended:
                tk.Label(
                    title_wrap,
                    text="推荐",
                    bg=COLORS["success"],
                    fg="#ffffff",
                    font=self.font_badge,
                    padx=8,
                    pady=2,
                ).grid(row=0, column=1, sticky="e", padx=(10, 0))

            detail_label = tk.Label(
                row,
                text=candidate.detail,
                bg=row_bg,
                fg=COLORS["muted"],
                font=self.font_meta,
                justify="left",
                anchor="w",
                wraplength=720,
            )
            detail_label.grid(row=1, column=2, sticky="ew", padx=(0, 12), pady=(0, 12))
            self._candidate_detail_labels.append(detail_label)

        self.root.after_idle(self._refresh_wrap_lengths)

    def save_config_to_disk(self) -> None:
        try:
            config = {
                "managed_rules": self._selected_rules(),
                "cleanup_scope": normalize_cleanup_scope(self.scope_value.get()),
                "cleanup_on_login": bool(self.cleanup_on_login_var.get()),
                "auto_start_helper": bool(self.auto_start_helper_var.get()),
                "meta": {
                    "version": 1,
                    "saved_at": utc_timestamp(),
                },
            }

            set_helper_auto_start(bool(self.auto_start_helper_var.get()))
            save_config(config)
            self.config_data = config
            self.status_var.set(f"已保存配置：{config_path()}")
            messagebox.showinfo("Proxy Guard", "设置已保存。")
        except Exception as exc:  # noqa: BLE001
            self.status_var.set(f"保存失败：{exc}")
            messagebox.showerror("Proxy Guard", f"保存失败：\n{exc}")

    def _selected_rules(self) -> list[dict[str, Any]]:
        selected_ids = {
            candidate_id
            for candidate_id, var in self.candidate_vars.items()
            if bool(var.get())
        }
        return [candidate.rule for candidate in self.candidates if candidate.candidate_id in selected_ids]


def normalize_cleanup_scope(value: str | None) -> str:
    if value == "ShutdownOnly":
        return "ShutdownAndRestart"
    if value in {"ShutdownAndRestart", "ShutdownRestartAndLogoff"}:
        return value
    return "ShutdownAndRestart"


def scope_value_to_label(value: str) -> str:
    for option_value, label in CLEANUP_SCOPE_OPTIONS:
        if option_value == normalize_cleanup_scope(value):
            return label
    return CLEANUP_SCOPE_OPTIONS[0][1]


def portable_mode() -> bool:
    return (base_dir() / PORTABLE_MARKER_FILE).is_file()


def base_dir() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent


def config_dir() -> Path:
    if portable_mode():
        return base_dir() / PORTABLE_CONFIG_DIR_NAME
    local_app_data = os.environ.get("LOCALAPPDATA")
    if local_app_data:
        return Path(local_app_data) / CONFIG_DIR_NAME
    return Path.home() / "AppData" / "Local" / CONFIG_DIR_NAME


def config_path() -> Path:
    return config_dir() / CONFIG_FILE_NAME


def load_config() -> dict[str, Any]:
    path = config_path()
    if not path.exists():
        return default_config()

    with path.open("r", encoding="utf-8") as handle:
        data = json.load(handle)

    config = default_config()
    config.update(data)
    config["cleanup_scope"] = normalize_cleanup_scope(config.get("cleanup_scope"))
    config["cleanup_on_login"] = bool(config.get("cleanup_on_login", False))
    config["auto_start_helper"] = bool(config.get("auto_start_helper", False))
    config["managed_rules"] = list(config.get("managed_rules", []))
    meta = dict(config.get("meta", {}))
    meta.setdefault("version", 1)
    meta.setdefault("saved_at", utc_timestamp())
    config["meta"] = meta
    return config


def default_config() -> dict[str, Any]:
    return {
        "managed_rules": [],
        "cleanup_scope": "ShutdownAndRestart",
        "cleanup_on_login": False,
        "auto_start_helper": False,
        "meta": {
            "version": 1,
            "saved_at": utc_timestamp(),
        },
    }


def save_config(data: dict[str, Any]) -> None:
    path = config_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as handle:
        json.dump(data, handle, ensure_ascii=False, indent=2)


def utc_timestamp() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def load_system_proxy_snapshot() -> dict[str, Any]:
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, INTERNET_SETTINGS_SUBKEY) as key:
        proxy_enable = query_registry_value(key, "ProxyEnable", 0)
        proxy_server = query_registry_value(key, "ProxyServer", None)
        auto_config_url = query_registry_value(key, "AutoConfigURL", None)

    return {
        "proxy_enable": bool(proxy_enable),
        "proxy_server": proxy_server,
        "auto_config_url": auto_config_url,
    }


def query_registry_value(key: Any, name: str, default: Any) -> Any:
    try:
        value, _value_type = winreg.QueryValueEx(key, name)
        return value
    except FileNotFoundError:
        return default


def is_helper_auto_start_enabled() -> bool:
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, RUN_SUBKEY) as key:
            winreg.QueryValueEx(key, RUN_VALUE_NAME)
            return True
    except FileNotFoundError:
        return False


def set_helper_auto_start(enabled: bool) -> None:
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, RUN_SUBKEY) as key:
        if enabled:
            helper_path = resolve_helper_path()
            winreg.SetValueEx(key, RUN_VALUE_NAME, 0, winreg.REG_SZ, f'"{helper_path}"')
        else:
            try:
                winreg.DeleteValue(key, RUN_VALUE_NAME)
            except FileNotFoundError:
                pass


def resolve_helper_path() -> Path:
    candidates = [
        base_dir() / HELPER_EXE_NAME,
        base_dir() / "target" / "release" / HELPER_EXE_NAME,
    ]

    for path in candidates:
        if path.exists():
            return path

    tried = "\n".join(str(path) for path in candidates)
    raise FileNotFoundError(
        "未找到 proxy_guard_helper.exe。请先构建 helper。\n\n已尝试位置：\n" + tried
    )


def scan_candidates_from_snapshot(snapshot: dict[str, Any]) -> list[Candidate]:
    candidates: list[Candidate] = []

    raw_proxy_server = str(snapshot.get("proxy_server") or "").strip()
    if raw_proxy_server:
        parsed = parse_manual_proxy(raw_proxy_server)
        if parsed:
            normalized = parsed.normalized()
            recommended = parsed.is_loopback_only()
            title = "分协议代理" if parsed.has_protocol_map else "手动代理"
            detail = normalized
            label = f"{title}: {detail}"
            candidates.append(
                Candidate(
                    candidate_id=f"manual::{normalized}",
                    title=title,
                    detail=detail,
                    recommended=recommended,
                    rule={
                        "kind": "manual_proxy",
                        "label": label,
                        "normalized_proxy_server": normalized,
                        "recommended": recommended,
                    },
                )
            )

    raw_pac_url = str(snapshot.get("auto_config_url") or "").strip()
    if raw_pac_url:
        normalized = normalize_pac_url(raw_pac_url)
        recommended = is_loopback_url(normalized)
        candidates.append(
            Candidate(
                candidate_id=f"pac::{normalized}",
                title="PAC",
                detail=normalized,
                recommended=recommended,
                rule={
                    "kind": "pac_url",
                    "label": f"PAC: {normalized}",
                    "normalized_url": normalized,
                    "recommended": recommended,
                },
            )
        )

    return candidates


def parse_manual_proxy(raw: str) -> ManualProxySpec | None:
    value = raw.strip()
    if not value:
        return None

    if "=" in value:
        entries_by_protocol: dict[str, tuple[str, int]] = {}
        for part in [item.strip() for item in value.split(";") if item.strip()]:
            if "=" not in part:
                return None
            protocol, endpoint = part.split("=", 1)
            parsed = parse_endpoint(endpoint.strip())
            if parsed is None:
                return None
            entries_by_protocol[protocol.strip().lower()] = parsed

        entries = [
            ManualProxyEntry(protocol=protocol, host=host, port=port)
            for protocol, (host, port) in entries_by_protocol.items()
        ]
        if not entries:
            return None
        return ManualProxySpec(has_protocol_map=True, entries=entries)

    parsed = parse_endpoint(value)
    if parsed is None:
        return None
    host, port = parsed
    return ManualProxySpec(
        has_protocol_map=False,
        entries=[ManualProxyEntry(protocol=None, host=host, port=port)],
    )


def parse_endpoint(value: str) -> tuple[str, int] | None:
    trimmed = value.strip()
    if not trimmed:
        return None

    if trimmed.startswith("[") and "]" in trimmed:
        host, remainder = trimmed[1:].split("]", 1)
        if not remainder.startswith(":"):
            return None
        try:
            port = int(remainder[1:])
        except ValueError:
            return None
        return host.lower(), port

    if ":" not in trimmed:
        return None
    host, port_text = trimmed.rsplit(":", 1)
    host = host.strip().lower()
    if not host:
        return None
    try:
        port = int(port_text.strip())
    except ValueError:
        return None
    return host, port


def normalize_pac_url(url: str) -> str:
    return url.strip().lower()


def is_loopback_url(url: str) -> bool:
    prefixes = [
        "http://127.0.0.1",
        "https://127.0.0.1",
        "http://localhost",
        "https://localhost",
        "http://[::1]",
        "https://[::1]",
    ]
    lower = url.strip().lower()
    return any(lower.startswith(prefix) for prefix in prefixes)


def is_loopback_host(host: str) -> bool:
    return host in {"127.0.0.1", "localhost", "::1"}


def is_rule_selected(rule: dict[str, Any], selected_rules: list[dict[str, Any]]) -> bool:
    kind = rule.get("kind")
    if kind == "manual_proxy":
        normalized = rule.get("normalized_proxy_server")
        return any(
            item.get("kind") == "manual_proxy"
            and item.get("normalized_proxy_server") == normalized
            for item in selected_rules
        )
    if kind == "pac_url":
        normalized = rule.get("normalized_url")
        return any(
            item.get("kind") == "pac_url" and item.get("normalized_url") == normalized
            for item in selected_rules
        )
    return False


def initial_loading_status_text() -> str:
    mode = (
        "当前模式：便携模式，配置保存在程序目录旁的 config 文件夹。"
        if portable_mode()
        else "当前模式：安装模式，配置保存在 %LOCALAPPDATA%\\ProxyGuard。"
    )
    return f"{mode}\n正在读取当前系统代理，请稍候…"


def initial_status_text(candidates: list[Candidate]) -> str:
    mode = (
        "当前模式：便携模式，配置保存在程序目录旁的 config 文件夹。"
        if portable_mode()
        else "当前模式：安装模式，配置保存在 %LOCALAPPDATA%\\ProxyGuard。"
    )
    if not candidates:
        return f"{mode}\n当前没有检测到系统代理候选项。"
    return f"{mode}\n已检测到 {len(candidates)} 个候选项。推荐优先勾选回环地址代理。"


def install_exception_hook() -> None:
    def report_callback_exception(
        _self: tk.Misc,
        exc_type: type[BaseException],
        exc_value: BaseException,
        exc_traceback: Any,
    ) -> None:
        detail = "".join(traceback.format_exception(exc_type, exc_value, exc_traceback))
        messagebox.showerror("Proxy Guard", detail)

    tk.Tk.report_callback_exception = report_callback_exception


def main() -> int:
    install_exception_hook()
    root = tk.Tk()
    ProxyGuardSetupApp(root)
    root.mainloop()
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:  # noqa: BLE001
        detail = traceback.format_exc()
        try:
            messagebox.showerror("Proxy Guard", detail)
        except Exception:  # noqa: BLE001
            pass
        raise
