from time import sleep
import serial
import numpy as np


def checksum_8(data):
    checksum = 0
    for byte in data:
        checksum += byte
    return checksum & 0xFF  # 保证结果在 0 到 255 之间


# CRC32 查找表
CRC_TAB = np.zeros(256, dtype=np.uint32)
polynomial = 0xEDB88320

for i in range(256):
    crc = i
    for _ in range(8):
        if crc & 1:
            crc = (crc >> 1) ^ polynomial
        else:
            crc >>= 1
    CRC_TAB[i] = crc

# 计算 CRC32
def crc32(data):
    crc = 0xFFFFFFFF
    for byte in data:
        crc = CRC_TAB[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return crc ^ 0xFFFFFFFF

'''
ser: serialport
type: application type
data: senddata
'''
def send(ser, check_type, data):
    # check_type == 0, use checksum_8
    # check_type == 1, use crc32
    
    # Calculate check from data[3]
    if check_type == 0:
        check = checksum_8(data=bytes(data[3:]))
        # convert to a u8 list with one element
        check = [check]
    elif check_type == 1:
        check = crc32(data=bytes(data[3:]))
        # convert check to 4 bytes(big end)
        check = [(check & 0xFF000000) >> 24, (check & 0x00FF0000) >> 16, (check & 0x0000FF00) >> 8, check & 0x000000FF]
    else:
        print("check_type error")
        return

    # append check to data
    data = data + check
    
    # send
    ser.write(data)
    
    
    

if __name__ == "__main__":
    ser = serial.Serial("/dev/ttyV1", 115200, write_timeout=10)

    # send request frame
    send(ser, 0, [0xEB, 0x90, 0x85, 0x00, 0x05, 0x05, 0xA0, 0x00, 0x00, 0x00])
    
    # send data frame
    # step 1. read the file
    with open("/home/shanmu/frames.bin", "rb") as f:
        data = f.read()
    
    # step 2. chunk the file with 1024
    chunk_size = 1024
    chunk_num = len(data) // chunk_size
    for i in range(chunk_num):
        # NOTE: All the bytes are in big end
        # 0xEB 0x90 0x85 [data_len: 2bytes] 0x05 0xA1 0x00 [chunk_order:2bytes] [chunk_num:2bytes] [data: max 1024bytes] [crc32: 4bytes]
        # data_len is the count of bytes from `0x05` to `[data: max 1024bytes]`
        # chunk_order is the order of chunk
        # chunk_num is the total number of chunks
        # data is the chunk data
        # crc32 is the crc32 of data
        # NOTE: the last chunk may not be 1024 bytes
        data_len = 1 + 1 + 1 + 2 + 2 + 1024
        send(ser, 1, [0xEB, 0x90, 0x85, (data_len & 0xFF00) >> 8, data_len & 0xFF, 0x05, 0xA1, (i & 0xFF00) >> 8, i & 0xFF] + list(data[i * chunk_size:(i + 1) * chunk_size]))
        
    # step 3. send the last chunk
    # 0xEB 0x90 0x85 [data_len: 2bytes] 0x05 0xA1 0x00 [chunk_order:2bytes] [chunk_num:2bytes] [data: max 1024bytes] [crc32: 4bytes]
    data_len = 1 + 1 + 1 + 2 + 2 + (len(data) - chunk_num * chunk_size)
    send(ser, 1, [0xEB, 0x90, 0x85, (data_len & 0xFF00) >> 8, data_len & 0xFF, 0x05, 0xA1, (chunk_num & 0xFF00) >> 8, chunk_num & 0xFF] + list(data[chunk_num * chunk_size:]))
        
    
    