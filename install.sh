#Prepare base env
export RUSTUP_INIT_SKIP_PATH_CHECK=yes

sudo apt update
sudo apt upgrade -y
sudo apt install gcc -y
sudo apt install libfontconfig -y
sudo apt install libfontconfig1-dev -y
sudo apt install dos2unix -y
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 80 -j ACCEPT

sudo apt install unzip -y
sudo apt install git -y
sudo curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

source $HOME/.cargo/env
rustup update
#Create swap file
#sudo swapoff /swapfile
#sudo fallocate -l 4G /swapfile
#sudo chmod 600 /swapfile
#sudo mkswap /swapfile
#sudo swapon /swapfile
#sudo free -h
#echo '/swapfile none swap sw 0 0' | sudo tee -a /etc/fstab

# Start install ppaass
sudo ps -ef | grep ppaass | grep -v grep | awk '{print $2}' | xargs sudo kill

sudo rm -rf /ppaass/build
sudo rm -rf /ppaass/sourcecode
# Build
sudo mkdir /ppaass
sudo mkdir /ppaass/sourcecode
sudo mkdir /ppaass/build
sudo mkdir /ppaass/build/resources

# Pull ppaass
cd /ppaass/sourcecode
sudo git clone https://github.com/quhxuxm/ppaass.git ppaass
sudo chmod 777 ppaass
cd /ppaass/sourcecode/ppaass
sudo git pull

cargo build --release

# ps -ef | grep gradle | grep -v grep | awk '{print $2}' | xargs kill -9
sudo cp -r /ppaass/sourcecode/ppaass/resources/ /ppaass/build/resources/
sudo cp /ppaass/sourcecode/ppaass/target/release/ppaass-proxy /ppaass/build
sudo cp /ppaass/sourcecode/ppaass/ppaass-proxy-start.sh /ppaass/build/

sudo chmod 777 /ppaass/build
cd /ppaass/build
ls -l

sudo chmod 777 ppaass-proxy
sudo chmod 777 *.sh
sudo dos2unix ./ppaass-proxy-start.sh

#Start with the low configuration by default
sudo nohup ./ppaass-proxy-start.sh >run.log 2>&1 &

ulimit -n 409600

