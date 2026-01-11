
import os
import asyncio
import logging
import datetime
from pathlib import Path
from typing import Optional, List
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from dotenv import load_dotenv

# 로깅 설정 (기존 logger가 있다면 활용)
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

class ObsidianMCPClient:
    def __init__(self):
        load_dotenv()
        self.server_path = os.getenv("OBSIDIAN_MCP_SERVER_EXECUTABLE")
        self.vault_dir = os.getenv("OBSIDIAN_VAULT_DIR")
        self.api_key = os.getenv("OBSIDIAN_API_KEY") # If needed by the server
        
        if not self.server_path:
            logger.warning("OBSIDIAN_MCP_SERVER_EXECUTABLE environment variable is not set. MCP features will be disabled.")
        if not self.vault_dir:
            logger.warning("OBSIDIAN_VAULT_DIR environment variable is not set. MCP features will be disabled.")

    async def _run_tool(self, tool_name: str, arguments: dict):
        if not self.server_path or not self.vault_dir:
            logger.error("MCP configuration missing. Cannot run tool.")
            return None

        server_params = StdioServerParameters(
            command=self.server_path, # e.g., "node" or path to executable
            args=[], # Add args if server_path is just the runtime (like 'node path/to/server.js')
            # If the user's provided python code example is correct, the server might be a direct executable or script
            env={
                "OBSIDIAN_API_KEY": self.api_key or "",
                "PATH": os.getenv("PATH")
            }
        )
        
        # User provided python code used `command=SERVER_PATH`. 
        # However, often node servers need ["path/to/script"].
        # We will assume SERVER_PATH is the full command or executable for now, 
        # but might need adjustment if it's "node build/index.js".
        # Let's check environment usage in config.py update later.
        
        try:
            async with stdio_client(server_params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()
                    result = await session.call_tool(tool_name, arguments)
                    return result
        except Exception as e:
            logger.error(f"MCP Tool execution failed: {tool_name}, args: {arguments}, error: {e}")
            return None

    async def save_stt_result(self, file_uuid: str, content: str, created_at: str = None):
        """
        Save STT result to Obsidian via MCP.
        Creates a new file if it doesn't exist, or appends if it does (though STT usually implies new file).
        """
        if not created_at:
            created_at = datetime.datetime.now().strftime("%Y-%m-%d %H%M%S")
        
        filename = f"{file_uuid}.md"
        full_path = f"{self.vault_dir}/{filename}" # Assuming flat structure or handle paths

        frontmatter = f"""---
author: 서요한
from:
  - "[[RecordRoute]]"
created: {created_at}
aliases:
---

"""
        full_content = frontmatter + content
        
        # Check if file exists (optional, or just try creating)
        # We'll try to get the file first to see if it exists
        
        logger.info(f"Saving STT result to Obsidian: {filename}")
        
        # Using list_vault_files to check existence might be heavy if vault is huge.
        # Instead, try 'get_vault_file' and catch error, or just 'create_vault_file' and handle error?
        # The user requested: "이미 해당 파일(UUID 동일)이 존재하는 경우 append 방식 사용"
        
        try:
            # Try to read to check existence
            path_check = await self._run_tool("get_vault_file", {"filename": full_path})
            
            if path_check and not path_check.is_error:
                # File exists, append
                logger.info(f"File {filename} exists. Appending STT content.")
                 # Note: Frontmatter is only for new files. 
                 # If appending, we probably just want the content?
                 # User said: "단, 이미 해당 파일(UUID 동일)이 존재하는 경우 append 방식 사용"
                await self._run_tool("append_to_vault_file", {
                    "filename": full_path,
                    "content": "\n\n" + content
                })
            else:
                # File does not exist, create
                logger.info(f"Creating new file {filename}")
                await self._run_tool("create_vault_file", {
                    "filename": full_path,
                    "content": full_content
                })
                
        except Exception as e:
            logger.error(f"Failed to save STT result: {e}")


    async def save_summary_result(self, file_uuid: str, summary_content: str):
        """
        Save Summary result to Obsidian via MCP.
        Appends to existing file or creates new with frontmatter.
        """
        created_at = datetime.datetime.now().strftime("%Y-%m-%d %H%M%S")
        filename = f"{file_uuid}.md"
        full_path = f"{self.vault_dir}/{filename}" 

        logger.info(f"Saving Summary result to Obsidian: {filename}")

        try:
             # Try to read to check existence
            path_check = await self._run_tool("get_vault_file", {"filename": full_path})
            
            if path_check and not path_check.is_error:
                # File exists, append
                logger.info(f"File {filename} exists. Appending Summary content.")
                await self._run_tool("append_to_vault_file", {
                    "filename": full_path,
                    "content": "\n\n" + summary_content
                })
            else:
                 # File does not exist, create
                logger.info(f"Creating new file {filename} for Summary")
                frontmatter = f"""---
author: 서요한
from:
  - "[[RecordRoute]]"
created: {created_at}
aliases:
---

"""
                await self._run_tool("create_vault_file", {
                    "filename": full_path,
                    "content": frontmatter + summary_content
                })

        except Exception as e:
            logger.error(f"Failed to save Summary result: {e}")

