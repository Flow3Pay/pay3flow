"""Local TLS-only GraphQL fixture. Never accesses an external network."""
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import json
import ssl
import sys

state_path, cert, key = sys.argv[1:]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        state = json.loads(Path(state_path).read_text())
        query = body["query"]
        if "blocks(" in query:
            data = {"blocks": [state["head"]]}
        elif "accounts(" in query:
            data = {"accounts": [state["single_key_account"]]}
        elif "transactions(" in query:
            data = {"transactions": state.get("receipts", [])}
        else:
            raise ValueError("Unexpected fixture query")
        payload = json.dumps({"data": data}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
tls.load_cert_chain(cert, key)
server.socket = tls.wrap_socket(server.socket, server_side=True)
print(server.server_address[1], flush=True)
server.serve_forever()
