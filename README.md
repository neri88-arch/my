# dotfiles — niri on Arch Linux

Minimal Arch Linux setup built around **niri** (a scrolling-tiling window manager for Wayland), aimed at being lightweight, fast, and keyboard-driven.

![Lock screen](./Wallpaper.jpg)

## Key features

- **Window manager**: [niri](https://github.com/YaLTeR/niri), scrolling-tiling on Wayland — columns instead of a fixed grid layout, gaps, a gradient focus ring, shadows, rounded corners, transparent background and blur on windows.
- **Minimalism**: `vim`, `alacritty`, `waybar`, `htop`, `swaylock` and `swaybg` are removed from the defaults in favor of the stack listed below.
- **CapsLock as an app-launch key**: CapsLock is remapped via XKB to `ISO_Level5_Shift` (not a true "Hyper" key — niri doesn't support that as a modifier name, see [Notes](#notes)) and used as a dedicated modifier to launch applications, without touching the window-management binds on `Mod` (Super).
- **Desktop shell**: [noctalia](https://github.com) spawned at startup together with the polkit authentication agent.
- **Networking**: Tor enabled as a service, DNS pinned to Quad9 with an immutable `resolv.conf` (`chattr +i`), IPv6 disabled, `ufw` firewall with deny incoming / allow outgoing.
- **User shell**: zsh + oh-my-zsh with the `powerlevel10k` theme, `zsh-autosuggestions`, `zsh-syntax-highlighting`, `zsh-autocomplete` plugins, and "modern unix" aliases (`eza`, `bat`, `zoxide`).

## Applications

| Category | App | Notes |
|---|---|---|
| Terminal | `kitty` | 80% opacity, MesloLGL Nerd Font |
| Text editor | `zed` (`zeditor`) | |
| Main browser | `zen-browser` | |
| Secondary browser / AI agent | `qutebrowser` | opens `chat.qwen.ai` as a dedicated agent |
| File manager | `dolphin` | |
| Image viewer | `gwenview` | |
| Media player | `mpv` | |
| System monitor | `btop` | |
| System fetch | `fastfetch` | |
| Login screen | `sddm` + `sddm-sugar-candy-git` theme | see [Manual steps](#manual-steps-after-installation) |
| Virtualization | `virt-manager`, `qemu`, `libvirtd`, `distrobox`, `podman` | |
| AUR helper | `paru` | installed automatically by the script |
| AI/ML | `llama.cpp` | cloned and built with CUDA support |

## Key bindings

CapsLock (`ISO_Level5_Shift`) + key → launches an application:

| Combo | Action |
|---|---|
| `CapsLock + Q` | Kitty (single instance) |
| `CapsLock + B` | Zen Browser |
| `CapsLock + Z` | Zed |
| `CapsLock + E` | Dolphin |
| `CapsLock + V` | virt-manager |
| `CapsLock + W` | qutebrowser → chat.qwen.ai |
| `CapsLock + H` | btop (inside Kitty) |

Main window management (`Mod` = Super):

| Combo | Action |
|---|---|
| `Mod + H/J/K/L` or arrows | Move focus between columns/windows |
| `Mod + Ctrl + H/J/K/L` | Move the window/column |
| `Mod + 1..9` | Go to workspace N |
| `Mod + F` | Maximize column |
| `Mod + Shift + F` | Fullscreen |
| `Mod + O` | Overview |
| `Mod + Return` | Close window |
| `Mod + V` | Floating window |
| `Super + Alt + L` | Lock screen (`swaylock`) |
| `PrintScreen` / `Ctrl+PrintScreen` / `Alt+PrintScreen` | Screenshot area/screen/window |

The full list of binds is in [`config.kdl`](./config.kdl).

## Installation

```bash
git clone https://github.com/neri88-arch/my.git && cd my && bash install.sh
```

The script asks for a temporary sudo password (removed automatically at the end, or if the script is interrupted), updates the system, installs packages from `pacman` and AUR (via `paru`), configures zsh, networking, the firewall, and copies `config.kdl` to `~/.config/niri/`.

## Manual steps after installation

A few things aren't applied automatically and need to be fixed by hand after the first reboot:

- **SDDM theme**: the script installs the `sddm-sugar-candy-git` theme but doesn't set it as the active one. It has to be selected manually by opening the graphical SDDM configuration tool installed via `paru` (check the installed packages for the exact tool name — something like `sddm-config-editor`/`sddm-conf`) and choosing `sugar-candy` as the greeter.
- **Login/lock screen wallpaper**: it isn't confirmed whether the `Wallpaper.jpg` file included in this repo gets copied automatically into the folder used by the greeter theme. Check after installation and, if needed, copy it manually and select it from the theme's settings.

## Notes

- Niri doesn't support `Hyper` as a modifier name in binds, which is why CapsLock is remapped to `ISO_Level5_Shift` via an XKB option (`lv5:caps_switch`) instead of a true Hyper layer.
- Keyboard layout: if it isn't "it" by default, uncomment `layout "it"` in `config.kdl`.
- The script removes `swaylock` from niri's default packages, but the `Super+Alt+L` bind in `config.kdl` still calls it — make sure `swaylock` stays installed (or update the bind) if you want keyboard screen locking to work.
- The script disables IPv6 and pins DNS to Quad9 with an immutable `resolv.conf`: to change it later you first need to remove the immutable attribute (`sudo chattr -i /etc/resolv.conf`).
