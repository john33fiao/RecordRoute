import asyncio
import os
import sys
from pathlib import Path
from dotenv import load_dotenv

# Ensure we can import modules from sttEngine
sys.path.append(str(Path(__file__).parent))

from mcp_client import ObsidianMCPClient
from config import get_obsidian_mcp_config

async def test_connection():
    print("Testing Obsidian MCP Connection...")
    
    # Load env (though checking config directly is also good)
    load_dotenv()
    
    config = get_obsidian_mcp_config()
    print(f"Config: {config}")
    
    if not config['server_executable']:
        print("❌ OBSIDIAN_MCP_SERVER_EXECUTABLE env var is missing!")
        return
    if not config['vault_dir']:
        print("❌ OBSIDIAN_VAULT_DIR env var is missing!")
        return

    client = ObsidianMCPClient()
    
    # Try a simple tool call, e.g., list files in vault root or get a file
    # We will try to list files in TARGET_DIR or just check availability
    
    print("\nAttempting to call 'list_vault_files' on vault root...")
    try:
        # Assuming list_vault_files takes 'directory' argument and vault_dir is absolute path
        # or relative to vault root? MCP implementation varies.
        # Usually Obsidian MCP takes relative path from vault root or just '/'?
        # Let's try '/'
        result = await client._run_tool("list_vault_files", {"directory": "/"})
        
        if result:
            print("✅ Connection Successful!")
            print(f"Result snippet: {str(result)[:200]}...")
        else:
            print("⚠️ Connection returned None (might be failure or empty).")
            
    except Exception as e:
        print(f"❌ Connection or Tool Execution Failed: {e}")

if __name__ == "__main__":
    asyncio.run(test_connection())
