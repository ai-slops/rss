#!/usr/bin/env python3
import argparse
import asyncio
import contextlib
import ssl


async def relay(
    reader: asyncio.StreamReader,
    writer: asyncio.StreamWriter,
) -> None:
    while data := await reader.read(64 * 1024):
        writer.write(data)
        await writer.drain()
    if writer.can_write_eof():
        writer.write_eof()
        await writer.drain()
    else:
        writer.close()


async def relay_server(reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
    await relay(reader, writer)


async def handle(
    client_reader: asyncio.StreamReader,
    client_writer: asyncio.StreamWriter,
    target_port: int,
) -> None:
    try:
        target_reader, target_writer = await asyncio.open_connection("127.0.0.1", target_port)
    except OSError:
        client_writer.close()
        await client_writer.wait_closed()
        return
    try:
        try:
            async with asyncio.TaskGroup() as group:
                group.create_task(relay(client_reader, target_writer))
                group.create_task(relay_server(target_reader, client_writer))
        except* (ConnectionError, asyncio.IncompleteReadError):
            pass
    finally:
        target_writer.close()
        client_writer.close()
        with contextlib.suppress(ConnectionError):
            await target_writer.wait_closed()
        with contextlib.suppress(ConnectionError):
            await client_writer.wait_closed()


async def main() -> None:
    parser = argparse.ArgumentParser(description="Local TLS front end for a controlled RTMP receiver")
    parser.add_argument("--cert", required=True)
    parser.add_argument("--key", required=True)
    parser.add_argument("--listen-port", required=True, type=int)
    parser.add_argument("--target-port", required=True, type=int)
    args = parser.parse_args()

    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(args.cert, args.key)
    server = await asyncio.start_server(
        lambda reader, writer: handle(reader, writer, args.target_port),
        "127.0.0.1",
        args.listen_port,
        ssl=context,
    )
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    asyncio.run(main())
