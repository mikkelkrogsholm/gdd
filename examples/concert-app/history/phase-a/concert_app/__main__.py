"""Run the reproducible local demo on loopback."""

import argparse
from pathlib import Path

from .api import make_server


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--database", type=Path, required=True)
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--host", choices=("127.0.0.1", "localhost"), default="127.0.0.1")
    args = parser.parse_args()
    server = make_server(args.database, args.host, args.port)
    print(f"Local Sessions: http://{args.host}:{server.server_port}", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
