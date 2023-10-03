#!/bin/bash
set -e
sudo ip tuntap add mode tap tap0
sudo sysctl net.ipv6.conf.tap0.disable_ipv6=1
sudo ip addr add 10.0.0.2/8 dev tap0 
sudo ip link set tap0 up
