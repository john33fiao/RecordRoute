from __future__ import annotations

import asyncio
import json
from typing import Any

import websockets


connected_clients: set[Any] = set()
websocket_loop = asyncio.new_event_loop()


async def _send_progress(task_id: str, message: str) -> None:
    data = json.dumps({"task_id": task_id, "message": message})
    if connected_clients:
        await asyncio.gather(
            *[client.send(data) for client in list(connected_clients) if not client.closed]
        )


def broadcast_progress(task_id: str, message: str) -> None:
    if websocket_loop.is_running():
        asyncio.run_coroutine_threadsafe(_send_progress(task_id, message), websocket_loop)


async def websocket_handler(websocket):
    connected_clients.add(websocket)
    try:
        async for _ in websocket:
            pass
    finally:
        connected_clients.discard(websocket)


def start_websocket_server() -> None:
    """Start the WebSocket server in its own asyncio event loop."""

    asyncio.set_event_loop(websocket_loop)

    async def run_server():
        async with websockets.serve(websocket_handler, "0.0.0.0", 8765):
            print("WebSocket server running on ws://localhost:8765")
            await asyncio.Future()  # run forever

    websocket_loop.run_until_complete(run_server())
