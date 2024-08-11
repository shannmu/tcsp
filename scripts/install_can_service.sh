#! /bin/bash

if [ $# -lt 2 ]; then
    echo "Usage: $0 <can_server> <can_id> [-i install]"
    exit 1
fi
can_server=$(realpath $1)
number="$2"
install=false

while [[ "$#" -gt 0 ]]; do
    case "$1" in
        -i)
            install=true
            shift
            ;;
        *)
            shift
            ;;
    esac
done

if [ ! -f "$can_server" ]; then
    echo "Error: Path '$can_server' does not exist."
    exit 1
fi

if [ ! -x "$can_server" ]; then
    echo "File '$can_server' is executable."
    exit 1
fi



if [[ "$number" =~ ^0x[0-9A-Fa-f]+$ ]]; then
    can_id=$number
    value=$(($number))
elif [[ "$number" =~ ^[0-9]+$ ]]; then
    can_id=$number
    value=$number
else
    echo "Error: '$number' is not a valid number"
    exit 1
fi

if [ "$value" -lt 0 ] || [ "$value" -gt 255 ]; then
    echo "Error: Number '$can_id' is out of range (0-255)."
    exit 1
fi


work_dir=$(dirname $can_server)

service="[Unit]
Description=Tiansuan Can Telemetry Service
After=network.target
StartLimitBurst=20
StartLimitIntervalSec=300

[Service]
Type=simple
ExecStart=$can_server $can_id
Restart=on-failure
RestartSec=5
User=root
Group=root
WorkingDirectory=$work_dir

[Install]
WantedBy=multi-user.target"

if $install; then
    sudo ln -s "$work_dir/tcsp_can.service" /etc/systemd/system/tcsp_can.service
    echo "$service" > "$work_dir/tcsp_can.service"
    sudo systemctl daemon-reload
    echo "type \'sudo systemctl enable tcsp_can.service\' to start server when booting system"
    echo "type \'sudo systemctl start tcsp_can.service\' to start service"
else
    echo "$service"
fi