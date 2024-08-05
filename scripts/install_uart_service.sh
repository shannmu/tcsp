#! /bin/bash

if [ $# -lt 1 ]; then
    echo "Usage: $0 <uart_server> [-i install]"
    exit 1
fi

uart_server=$(realpath $1)
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

if [ ! -f "$uart_server" ]; then
    echo "Error: Path '$uart_server' does not exist."
    exit 1
fi

if [ ! -x "$uart_server" ]; then
    echo "File '$uart_server' is executable."
    exit 1
fi

work_dir=$(dirname $uart_server)

service="[Unit]
Description=Tiansuan Uart Telemetry Service
After=network.target
StartLimitBurst=20
StartLimitIntervalSec=300

[Service]
Type=simple
ExecStart=$uart_server
Restart=on-failure
RestartSec=5
User=root
Group=root
WorkingDirectory=$work_dir

[Install]
WantedBy=multi-user.target"

if $install; then
    sudo ln -s "$work_dir/tcsp_uart.service" /etc/systemd/system/tcsp_uart.service
    echo "$service" > "$work_dir/tcsp_uart.service"
else
    echo "$service"
fi