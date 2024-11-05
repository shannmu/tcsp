from time import sleep
import serial



ser = serial.Serial("/dev/ttyV1", 115200, write_timeout=10)

# 0xeb 0x90 0x00 0x00 0x03 0x05 0xc0 0x00 0xc8
ser.write(b"\xeb\x90\x00\x00\x03\x05\xc0\x00\xc8")
sleep(5)
data = ser.read(1040)
print(' '.join(f'{byte:02x}' for byte in data))
print("\n")

# 0xeb 0x90 0x85 0x00 0x06 0x05 0xc0 0x00 0x00 0x00 0xaa 0x75
ser.write(b"\xeb\x90\x85\x00\x06\x05\xc0\x00\x00\x00\xaa\x75")
sleep(5)
data = ser.read(1040)
print(' '.join(f'{byte:02x}' for byte in data))
print("\n")

# 0xeb 0x90 0x85 0x00 0x06 0x05 0xc0 0x00 0x00 0x01 0xaa 0x76
ser.write(b"\xeb\x90\x85\x00\x06\x05\xc0\x00\x00\x01\xaa\x76")
sleep(5)
data = ser.read(1040)
print(' '.join(f'{byte:02x}' for byte in data))
print("\n")

ser.write(b"\xeb\x90\x85\x00\x06\x05\xc0\x00\x00\x02\xaa\x77")
sleep(5)
data = ser.read(1040)
print(' '.join(f'{byte:02x}' for byte in data))
print("\n")

ser.write(b"\xeb\x90\x85\x00\x06\x05\xc0\x00\x00\x03\xaa\x78")
sleep(5)
data = ser.read(1040)
print(' '.join(f'{byte:02x}' for byte in data))
print("\n")

ser.write(b"\xeb\x90\x85\x00\x06\x05\xc0\x00\x00\x04\xaa\x79")
sleep(5)
data = ser.read(1040)
print(' '.join(f'{byte:02x}' for byte in data))
print("\n")
