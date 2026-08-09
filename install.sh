#!/bin/bash

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

sudo -v

echo "$USER ALL=(ALL) NOPASSWD: ALL" | sudo tee /etc/sudoers.d/99-temp-install >/dev/null
sudo chmod 440 /etc/sudoers.d/99-temp-install

while true; do sudo -n true; sleep 60; kill -0 "$$" || exit; done 2>/dev/null &
SUDO_PID=$!
trap 'kill $SUDO_PID 2>/dev/null; sudo rm -f /etc/sudoers.d/99-temp-install' EXIT INT TERM

set -e

sudo pacman -Syu --noconfirm

sudo pacman -S --needed --noconfirm base-devel distrobox virt-manager qemu podman tor zsh ttf-meslo-nerd git cmake fastfetch sl devtools cuda torsocks
sudo pacman -S --needed --noconfirm kitty unzip polkit-kde-agent qemu-desktop virt-viewer uv bat eza gwenview mpv btop qutebrowser dolphin zoxide zed

git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si --noconfirm && cd .. && rm -rf paru

paru -S --needed --noconfirm obfs4proxy-bin zen-browser-bin zsh-theme-powerlevel10k-git sddm-conf-git oh-my-pi-bin sddm-sugar-candy-git noctalia 

sudo pacman -Rsn --noconfirm vim vim-runtime alacritty waybar htop swaylock swaybg

sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended

cd ~/.oh-my-zsh/custom/plugins/
git clone https://github.com/zsh-users/zsh-autosuggestions
git clone https://github.com/zsh-users/zsh-syntax-highlighting
git clone https://github.com/marlonrichert/zsh-autocomplete
cd ~

echo 'source /usr/share/zsh-theme-powerlevel10k/powerlevel10k.zsh-theme' >> ~/.zshrc

sed -i '/^plugins=/c\plugins=(git zsh-autosuggestions zsh-syntax-highlighting zsh-autocomplete)' ~/.zshrc

printf "\n# Modern Unix Aliases\nalias ls=\"eza --icons\"\nalias cat=\"bat\"\nalias pac='sudo pacman -S --needed'\n\n# Zoxide\neval \"\$(zoxide init zsh --cmd cd)\"\n" >> ~/.zshrc

sudo mkdir -p /etc/NetworkManager/conf.d
printf '[main]\ndns=none\n' | sudo tee /etc/NetworkManager/conf.d/dns.conf >/dev/null

printf 'nameserver 9.9.9.9\noptions edns0\n' | sudo tee /etc/resolv.conf >/dev/null

printf 'net.ipv6.conf.all.disable_ipv6 = 1\nnet.ipv6.conf.default.disable_ipv6 = 1\nnet.ipv6.conf.lo.disable_ipv6 = 1\n' | sudo tee /etc/sysctl.d/99-disable-ipv6.conf >/dev/null

mkdir -p ~/.config/kitty
printf 'background_opacity 0.8\nfont_family      family="MesloLGL Nerd Font"\nbold_font        auto\nitalic_font      auto\nbold_italic_font auto\n' >> ~/.config/kitty/kitty.conf

sudo systemctl enable --now tor libvirtd iptables

sudo chattr +i /etc/resolv.conf

git clone https://github.com/ggerganov/llama.cpp
cd llama.cpp && cmake -B build -DGGML_CUDA=ON && cmake --build build --config Release -j$(nproc)

cd ~

cp "$SCRIPT_DIR/config.kdl" ~/.config/niri/

sudo ufw default deny incoming
sudo ufw default allow outgoing 
sudo ufw enable

sudo chsh -s "$(which zsh)" "$USER"

echo "Installazione completata con successo! Riavvia la sessione o esegui 'zsh'."

echo ""
read -p "Do you want to reboot the system now? [Y/n]: " response

case "$response" in
    [nN][oO]|[nN])
        echo "Reboot canceled. Remember to reboot manually later to apply all changes!"
        ;;
    *)
        echo "Rebooting the system now..."
        sudo reboot
        ;;
esac
