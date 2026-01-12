"""
Obsidian MCP 통합 모듈

STT 및 요약 텍스트를 Obsidian Vault에 자동으로 전송하는 기능 제공
- STT 완료 시: UUID 파일명으로 마크다운 파일 생성
- 요약 완료 시: 동일 UUID 파일에 요약 내용 추가 (append)
"""

import asyncio
import os
from datetime import datetime
from pathlib import Path
from typing import Optional, Dict, Any
from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client
from dotenv import load_dotenv

# 환경 변수 로드
load_dotenv()


class ObsidianMCPIntegration:
    """Obsidian MCP 서버와 통신하여 파일을 생성/업데이트하는 클래스"""

    def __init__(self):
        """
        환경변수에서 설정 로드:
        - OBSIDIAN_MCP_ENABLED: MCP 사용 여부 (true/false)
        - OBSIDIAN_MCP_SERVER_PATH: MCP 서버 실행 파일 경로
        - OBSIDIAN_API_KEY: Obsidian API 키
        - OBSIDIAN_VAULT_FOLDER: Vault 내 저장 폴더 경로
        """
        self.enabled = os.getenv("OBSIDIAN_MCP_ENABLED", "false").lower() == "true"
        self.server_path = os.getenv("OBSIDIAN_MCP_SERVER_PATH")
        self.api_key = os.getenv("OBSIDIAN_API_KEY")
        self.vault_folder = os.getenv("OBSIDIAN_VAULT_FOLDER", "RecordRoute")

        # 설정 검증
        if self.enabled:
            if not self.server_path:
                print("[Obsidian MCP] WARNING: OBSIDIAN_MCP_SERVER_PATH가 설정되지 않았습니다.")
                self.enabled = False
            if not self.api_key:
                print("[Obsidian MCP] WARNING: OBSIDIAN_API_KEY가 설정되지 않았습니다.")
                self.enabled = False

    def _generate_frontmatter(self, filename: str, uuid: str, created_at: datetime) -> str:
        """
        YAML frontmatter 생성

        Args:
            filename: 원본 파일명 (표시용)
            uuid: 파일 UUID (검색용)
            created_at: 파일 생성 시각

        Returns:
            YAML frontmatter 문자열
        """
        return f"""---
author: 서요한
from:
  - "[[RecordRoute]]"
created: {created_at.strftime("%Y-%m-%d %H%M%S")}
aliases:
  - {filename}
  - {uuid}
---

"""

    async def _get_server_params(self) -> StdioServerParameters:
        """MCP 서버 파라미터 생성"""
        # Windows 경로 정규화
        import platform
        server_path = self.server_path
        if platform.system() == "Windows":
            # Windows 경로를 정규화 (백슬래시 유지)
            server_path = os.path.normpath(server_path)

        return StdioServerParameters(
            command=server_path,
            args=[],
            env={
                "OBSIDIAN_API_KEY": self.api_key,
                "PATH": os.getenv("PATH", "")
            }
        )

    async def _file_exists(self, session: ClientSession, uuid: str) -> bool:
        """
        Obsidian Vault에 파일이 존재하는지 확인

        Args:
            session: MCP 클라이언트 세션
            uuid: 파일 UUID

        Returns:
            파일 존재 여부
        """
        filename = f"{self.vault_folder}/{uuid}.md"
        try:
            await session.call_tool("get_vault_file", {"filename": filename})
            return True
        except Exception:
            return False

    async def send_stt_to_obsidian(
        self,
        uuid: str,
        stt_text: str,
        original_filename: str,
        created_at: Optional[datetime] = None
    ) -> Dict[str, Any]:
        """
        STT 텍스트를 Obsidian에 전송

        - 파일이 없으면: frontmatter + STT 텍스트로 새 파일 생성
        - 파일이 있으면: STT 텍스트를 append

        Args:
            uuid: 파일 UUID (파일명으로 사용)
            stt_text: STT 변환된 텍스트
            original_filename: 원본 파일명 (frontmatter aliases용)
            created_at: 파일 생성 시각 (None이면 현재 시각)

        Returns:
            결과 딕셔너리 {"success": bool, "message": str, "action": str}
        """
        if not self.enabled:
            return {
                "success": False,
                "message": "Obsidian MCP가 비활성화되어 있습니다.",
                "action": "skipped"
            }

        if created_at is None:
            created_at = datetime.now()

        filename = f"{self.vault_folder}/{uuid}.md"

        try:
            server_params = await self._get_server_params()

            async with stdio_client(server_params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()

                    # 파일 존재 확인
                    file_exists = await self._file_exists(session, uuid)

                    if file_exists:
                        # 파일이 이미 있으면 STT 텍스트만 append
                        content = f"\n\n## STT 원문\n\n{stt_text}\n"
                        await session.call_tool("append_to_vault_file", {
                            "filename": filename,
                            "content": content
                        })
                        action = "appended"
                        message = f"STT 텍스트가 기존 파일에 추가되었습니다: {filename}"
                    else:
                        # 새 파일 생성 (frontmatter + STT)
                        frontmatter = self._generate_frontmatter(original_filename, uuid, created_at)
                        content = f"{frontmatter}## STT 원문\n\n{stt_text}\n"

                        await session.call_tool("create_vault_file", {
                            "filename": filename,
                            "content": content
                        })
                        action = "created"
                        message = f"새 파일이 생성되었습니다: {filename}"

                    print(f"[Obsidian MCP] ✓ {message}")
                    return {
                        "success": True,
                        "message": message,
                        "action": action,
                        "filename": filename
                    }

        except FileNotFoundError as e:
            error_msg = f"MCP 서버를 찾을 수 없습니다: {self.server_path}"
            print(f"[Obsidian MCP] ✗ {error_msg}")
            print(f"[Obsidian MCP] 힌트: OBSIDIAN_MCP_SERVER_PATH 환경변수를 확인하세요.")
            return {
                "success": False,
                "message": error_msg,
                "action": "failed"
            }
        except Exception as e:
            error_msg = f"Obsidian MCP 전송 실패: {str(e)}"
            print(f"[Obsidian MCP] ✗ {error_msg}")
            import traceback
            print(f"[Obsidian MCP] 디버그 정보:")
            print(f"  - 서버 경로: {self.server_path}")
            print(f"  - Vault 폴더: {self.vault_folder}")
            print(f"  - 오류 상세: {traceback.format_exc()}")
            return {
                "success": False,
                "message": error_msg,
                "action": "failed"
            }

    async def send_summary_to_obsidian(
        self,
        uuid: str,
        summary_text: str,
        original_filename: str = "unknown",
        created_at: Optional[datetime] = None
    ) -> Dict[str, Any]:
        """
        요약 텍스트를 Obsidian에 전송

        - 파일이 있으면: 요약 텍스트를 append
        - 파일이 없으면: frontmatter + 요약 텍스트로 새 파일 생성 (STT 스킵된 케이스)

        Args:
            uuid: 파일 UUID
            summary_text: 요약된 텍스트
            original_filename: 원본 파일명
            created_at: 파일 생성 시각

        Returns:
            결과 딕셔너리
        """
        if not self.enabled:
            return {
                "success": False,
                "message": "Obsidian MCP가 비활성화되어 있습니다.",
                "action": "skipped"
            }

        if created_at is None:
            created_at = datetime.now()

        filename = f"{self.vault_folder}/{uuid}.md"

        try:
            server_params = await self._get_server_params()

            async with stdio_client(server_params) as (read, write):
                async with ClientSession(read, write) as session:
                    await session.initialize()

                    # 파일 존재 확인
                    file_exists = await self._file_exists(session, uuid)

                    if file_exists:
                        # 파일이 있으면 요약만 append
                        content = f"\n\n## 요약\n\n{summary_text}\n"
                        await session.call_tool("append_to_vault_file", {
                            "filename": filename,
                            "content": content
                        })
                        action = "appended"
                        message = f"요약이 기존 파일에 추가되었습니다: {filename}"
                    else:
                        # 파일이 없으면 새로 생성 (frontmatter + 요약)
                        # STT 없이 바로 요약된 케이스
                        frontmatter = self._generate_frontmatter(original_filename, uuid, created_at)
                        content = f"{frontmatter}## 요약\n\n{summary_text}\n"

                        await session.call_tool("create_vault_file", {
                            "filename": filename,
                            "content": content
                        })
                        action = "created"
                        message = f"새 파일이 생성되었습니다 (요약만): {filename}"

                    print(f"[Obsidian MCP] ✓ {message}")
                    return {
                        "success": True,
                        "message": message,
                        "action": action,
                        "filename": filename
                    }

        except FileNotFoundError as e:
            error_msg = f"MCP 서버를 찾을 수 없습니다: {self.server_path}"
            print(f"[Obsidian MCP] ✗ {error_msg}")
            print(f"[Obsidian MCP] 힌트: OBSIDIAN_MCP_SERVER_PATH 환경변수를 확인하세요.")
            return {
                "success": False,
                "message": error_msg,
                "action": "failed"
            }
        except Exception as e:
            error_msg = f"Obsidian MCP 전송 실패: {str(e)}"
            print(f"[Obsidian MCP] ✗ {error_msg}")
            import traceback
            print(f"[Obsidian MCP] 디버그 정보:")
            print(f"  - 서버 경로: {self.server_path}")
            print(f"  - Vault 폴더: {self.vault_folder}")
            print(f"  - 오류 상세: {traceback.format_exc()}")
            return {
                "success": False,
                "message": error_msg,
                "action": "failed"
            }

    def is_enabled(self) -> bool:
        """MCP 통합이 활성화되어 있는지 확인"""
        return self.enabled


# 동기 래퍼 함수들 (기존 동기 코드에서 사용)
def send_stt_to_obsidian_sync(
    uuid: str,
    stt_text: str,
    original_filename: str,
    created_at: Optional[datetime] = None
) -> Dict[str, Any]:
    """
    STT 텍스트를 Obsidian에 전송 (동기 버전)

    Args:
        uuid: 파일 UUID
        stt_text: STT 텍스트
        original_filename: 원본 파일명
        created_at: 생성 시각

    Returns:
        결과 딕셔너리
    """
    integration = ObsidianMCPIntegration()
    return asyncio.run(integration.send_stt_to_obsidian(
        uuid, stt_text, original_filename, created_at
    ))


def send_summary_to_obsidian_sync(
    uuid: str,
    summary_text: str,
    original_filename: str = "unknown",
    created_at: Optional[datetime] = None
) -> Dict[str, Any]:
    """
    요약 텍스트를 Obsidian에 전송 (동기 버전)

    Args:
        uuid: 파일 UUID
        summary_text: 요약 텍스트
        original_filename: 원본 파일명
        created_at: 생성 시각

    Returns:
        결과 딕셔너리
    """
    integration = ObsidianMCPIntegration()
    return asyncio.run(integration.send_summary_to_obsidian(
        uuid, summary_text, original_filename, created_at
    ))


if __name__ == "__main__":
    # 테스트 코드
    print("Obsidian MCP Integration Test")
    print("=" * 50)

    integration = ObsidianMCPIntegration()
    print(f"MCP Enabled: {integration.is_enabled()}")
    print(f"Server Path: {integration.server_path}")
    print(f"Vault Folder: {integration.vault_folder}")

    if integration.is_enabled():
        # 테스트 UUID
        test_uuid = "test-" + datetime.now().strftime("%Y%m%d-%H%M%S")

        print(f"\nTest UUID: {test_uuid}")

        # STT 전송 테스트
        print("\n1. STT 전송 테스트...")
        result = asyncio.run(integration.send_stt_to_obsidian(
            uuid=test_uuid,
            stt_text="이것은 테스트 STT 텍스트입니다.",
            original_filename="test_audio.m4a"
        ))
        print(f"Result: {result}")

        # 요약 전송 테스트
        print("\n2. 요약 전송 테스트...")
        result = asyncio.run(integration.send_summary_to_obsidian(
            uuid=test_uuid,
            summary_text="## 테스트 요약\n\n- 핵심 내용 1\n- 핵심 내용 2",
            original_filename="test_audio.m4a"
        ))
        print(f"Result: {result}")
    else:
        print("\nMCP가 비활성화되어 있어 테스트를 건너뜁니다.")
