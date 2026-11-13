#!/usr/bin/env python3
"""Tiny local SMTP server for tests/development: appends every received message to a file.
   usage: smtp-sink.py [port] [outfile]   (requires: pip install aiosmtpd)"""
import asyncio, sys
from aiosmtpd.controller import Controller

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 1025
OUT = sys.argv[2] if len(sys.argv) > 2 else "/tmp/smtp-sink.log"

class Handler:
    async def handle_DATA(self, server, session, envelope):
        with open(OUT, "ab") as f:
            f.write(b"=== MESSAGE ===\nMAIL FROM:" + envelope.mail_from.encode() + b"\nRCPT TO:" + ",".join(envelope.rcpt_tos).encode() + b"\n")
            f.write(envelope.content + b"\n")
        return "250 OK"

c = Controller(Handler(), hostname="127.0.0.1", port=PORT)
c.start()
print(f"smtp sink on 127.0.0.1:{PORT} -> {OUT}", flush=True)
try:
    asyncio.get_event_loop().run_forever()
except KeyboardInterrupt:
    c.stop()
