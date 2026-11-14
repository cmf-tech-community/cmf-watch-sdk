"""TCP bridge to the running app over the K channel (docs/abi-1.4.md): hex lines both ways, as `scripts/sim --tcp`."""
import asyncio

from . import channel


async def serve(port: int, address: str | None = None, hci_port: int | None = None, ready=None) -> None:
    async def client(reader, writer):
        ch = channel.Channel(address=address, hci_port=hci_port, channel_key=True)

        async def down():
            while True:
                m = await ch.recv()
                writer.write(m.data.hex().encode() + b"\n")
                await writer.drain()

        task = asyncio.create_task(down())
        try:
            while line := await reader.readline():
                await ch.send(bytes.fromhex(line.decode().strip()))
        finally:
            task.cancel()
            await ch.drop()
            writer.close()

    server = await asyncio.start_server(client, "127.0.0.1", port)
    if ready:
        ready()
    async with server:
        await server.serve_forever()
