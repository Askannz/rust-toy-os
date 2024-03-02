import socket

HOST = "127.0.0.1"
PORT = 1235

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.bind((HOST, PORT))
    s.listen()
    conn, addr = s.accept()
    with conn:
        print(f"Connected by {addr}")
        #data = conn.recv(4096)
        #print(data.hex())
        conn.send("Hello".encode())