"""WebSocket server for real-time progress updates."""

import asyncio
import json
import os
import websockets


# WebSocket server setup for real-time progress updates
connected_clients = set()
websocket_loop = asyncio.new_event_loop()


async def _send_progress(task_id: str, message: str) -> None:
    """Send progress update to all connected WebSocket clients."""
    data = json.dumps({"task_id": task_id, "message": message})
    if connected_clients:
        await asyncio.gather(
            *[client.send(data) for client in list(connected_clients) if not client.closed]
        )


def broadcast_progress(task_id: str, message: str) -> None:
    """Broadcast progress update to all connected clients."""
    if websocket_loop.is_running():
        asyncio.run_coroutine_threadsafe(_send_progress(task_id, message), websocket_loop)


async def websocket_handler(websocket) -> None:
    """Handle WebSocket connections."""
    connected_clients.add(websocket)
    try:
        async for _ in websocket:
            pass
    finally:
        connected_clients.discard(websocket)


def start_websocket_server() -> None:
    """Start the WebSocket server in its own asyncio event loop."""
    asyncio.set_event_loop(websocket_loop)

    # Get WebSocket port from environment variable or use default
    websocket_port = int(os.getenv('WEBSOCKET_PORT', '8765'))
    websocket_host = os.getenv('WEBSOCKET_HOST', '0.0.0.0')

    async def run_server():
        async with websockets.serve(websocket_handler, websocket_host, websocket_port):
            print(f"WebSocket server running on ws://localhost:{websocket_port}")
            await asyncio.Future()  # run forever

    websocket_loop.run_until_complete(run_server())
