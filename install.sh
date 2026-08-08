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

sudo pacman -S --needed --noconfirm base-devel distrobox virt-manager qemu podman tor zsh ttf-meslo-nerd git cmake fastfetch sl devtools neovim cuda kitty unzip polkit-kde-agent qemu-desktop virt-viewer uv bat eza zoxide gwenview mpv btop qutebrowser

git clone https://aur.archlinux.org/paru.git && cd paru && makepkg -si --noconfirm && cd .. && rm -rf paru

paru -S --needed --noconfirm obfs4proxy zen-browser zsh-theme-powerlevel10k-git sddm-conf oh-my-pi-bin sddm-theme-tokyo-night-git

sudo pacman -Rsn vim vim-runtime alacritty waybar htop 

CHSH=no RUNZSH=no KEEP_ZSHRC=yes sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)" "" --unattended

cd ~/.oh-my-zsh/custom/plugins/
git clone https://github.com/zsh-users/zsh-autosuggestions
git clone https://github.com/zsh-users/zsh-syntax-highlighting
git clone https://github.com/marlonrichert/zsh-autocomplete
cd -

echo 'source /usr/share/zsh-theme-powerlevel10k/powerlevel10k.zsh-theme' >> ~/.zshrc

sed -i '/^plugins=/c\plugins=(git zsh-autosuggestions zsh-syntax-highlighting zsh-autocomplete)' ~/.zshrc

printf '\n# Modern Unix Aliases\nalias ls="eza --icons"\nalias cat="bat"\neval "$(zoxide init zsh --cmd cd)"\n' >> ~/.zshrc

printf "\nalias pac='sudo pacman -S --needed'\nalias toron='sudo systemctl enable --now tor-firewall.service'\nalias toroff='sudo systemctl disable --now tor-firewall.service'\n" >> ~/.zshrc

printf "SocksPort 9050\nVirtualAddrNetworkIPv4 10.192.0.0/10\nAutomapHostsOnResolve 1\nTransPort 9040\nDNSPort 9053\n#UseBridges 1\n#ClientTransportPlugin obfs4 exec /usr/bin/obfs4proxy\n#Bridge\n#Bridge\n" | sudo tee -a /etc/tor/torrc >/dev/null

sudo mkdir -p /etc/NetworkManager/conf.d
printf '[main]\ndns=none\n' | sudo tee /etc/NetworkManager/conf.d/dns.conf >/dev/null

printf 'nameserver 9.9.9.9\noptions edns0\n' | sudo tee /etc/resolv.conf >/dev/null

printf 'net.ipv6.conf.all.disable_ipv6 = 1\nnet.ipv6.conf.default.disable_ipv6 = 1\nnet.ipv6.conf.lo.disable_ipv6 = 1\n' | sudo tee /etc/sysctl.d/99-disable-ipv6.conf >/dev/null

mkdir -p ~/.config/kitty && echo "background_opacity 0.8" >> ~/.config/kitty/kitty.conf

sudo systemctl enable --now tor libvirtd iptables

sudo chattr +i /etc/resolv.conf

mkdir Models
mkdir Project

git clone https://github.com/snowarch/inir.git &&
cd inir && ./setup install -y

git clone https://github.com/ggerganov/llama.cpp
cd llama.cpp && cmake -B build -DGGML_CUDA=ON && cmake --build build --config Release -j$(nproc)

cd ~

cp -r "$SCRIPT_DIR/nvim" ~/.config/
cp -r "$SCRIPT_DIR/wallpaper" ~/Pictures/
cp "$SCRIPT_DIR/config.kdl" ~/.config/niri/

sudo ufw default deny incoming
sudo ufw default allow outgoing 

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
