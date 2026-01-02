# ollama_utils.py
import logging
import os
import subprocess
import sys
import time
from typing import Optional, Tuple
import requests
import platform

try:
    import ollama
except ImportError:
    ollama = None

def check_ollama_server() -> Tuple[bool, str]:
    """
    Ollama 서버가 실행 중인지 확인합니다.

    Returns:
        Tuple[bool, str]: (서버 실행 여부, 상태 메시지)
    """
    # Get Ollama base URL from environment variable or use default
    ollama_base_url = os.getenv('OLLAMA_BASE_URL', 'http://localhost:11434')

    try:
        # HTTP 요청으로 ollama 서버 상태 확인
        response = requests.get(f"{ollama_base_url}/api/version", timeout=5)
        if response.status_code == 200:
            return True, "Ollama 서버가 정상적으로 실행 중입니다."
    except requests.exceptions.ConnectionError:
        return False, "Ollama 서버에 연결할 수 없습니다. 서버가 실행되지 않았거나 다른 포트를 사용 중일 수 있습니다."
    except requests.exceptions.Timeout:
        return False, "Ollama 서버 응답 시간이 초과되었습니다."
    except Exception as e:
        return False, f"Ollama 서버 상태 확인 중 오류 발생: {str(e)}"
    
    return False, "Ollama 서버 상태를 확인할 수 없습니다."

def start_ollama_server() -> Tuple[bool, str]:
    """
    Ollama 서버를 시작합니다.
    
    Returns:
        Tuple[bool, str]: (시작 성공 여부, 메시지)
    """
    try:
        # ollama serve 명령어 실행
        logging.info("Ollama 서버를 시작합니다...")
        
        if platform.system() == "Windows":
            # Windows에서는 새 창에서 실행
            subprocess.Popen(
                ["ollama", "serve"],
                creationflags=subprocess.CREATE_NEW_CONSOLE,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL
            )
        else:
            # Unix/macOS에서는 백그라운드 실행
            subprocess.Popen(
                ["ollama", "serve"],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                preexec_fn=lambda: None
            )
        
        # 서버 시작을 위해 잠시 대기
        time.sleep(3)
        
        # 시작 후 상태 확인
        is_running, message = check_ollama_server()
        if is_running:
            return True, "Ollama 서버가 성공적으로 시작되었습니다."
        else:
            return False, f"Ollama 서버 시작 후에도 연결할 수 없습니다: {message}"
            
    except FileNotFoundError:
        return False, "ollama 명령어를 찾을 수 없습니다. Ollama가 설치되어 있는지 확인하고 PATH에 추가되어 있는지 확인하세요."
    except Exception as e:
        return False, f"Ollama 서버 시작 중 오류 발생: {str(e)}"

def ensure_ollama_server(auto_start: bool = True) -> Tuple[bool, str]:
    """
    Ollama 서버가 실행 중인지 확인하고, 필요시 시작합니다.
    
    Args:
        auto_start: 서버가 실행되지 않은 경우 자동으로 시작할지 여부
        
    Returns:
        Tuple[bool, str]: (서버 사용 가능 여부, 상태 메시지)
    """
    # 먼저 서버 상태 확인
    is_running, message = check_ollama_server()
    
    if is_running:
        return True, message
    
    if not auto_start:
        return False, f"{message}\n수동으로 'ollama serve' 명령어를 실행해주세요."
    
    # 서버 시작 시도
    logging.info("Ollama 서버가 실행되지 않음. 자동으로 시작을 시도합니다...")
    start_success, start_message = start_ollama_server()
    
    if start_success:
        return True, start_message
    else:
        return False, f"서버 자동 시작 실패: {start_message}\n수동으로 'ollama serve' 명령어를 실행해주세요."

def check_ollama_model_available(model_name: str) -> Tuple[bool, str]:
    """
    지정된 모델이 Ollama에서 사용 가능한지 확인합니다.
    
    Args:
        model_name: 확인할 모델 이름
        
    Returns:
        Tuple[bool, str]: (모델 사용 가능 여부, 메시지)
    """
    if ollama is None:
        return False, "ollama 패키지가 설치되지 않았습니다."
    
    # 서버 실행 여부 먼저 확인
    server_ok, server_msg = ensure_ollama_server()
    if not server_ok:
        return False, f"Ollama 서버 문제: {server_msg}"
    
    try:
        models = ollama.list()
        available_models = [model['name'] for model in models.get('models', [])]
        
        # 정확한 모델명 또는 태그 포함 모델명으로 확인
        model_found = any(
            model_name == model or model.startswith(f"{model_name}:")
            for model in available_models
        )
        
        if model_found:
            return True, f"모델 '{model_name}'이 사용 가능합니다."
        else:
            available_str = ", ".join(available_models) if available_models else "없음"
            return False, f"모델 '{model_name}'을 찾을 수 없습니다. 사용 가능한 모델: {available_str}"
            
    except Exception as e:
        return False, f"모델 목록 확인 중 오류 발생: {str(e)}"

def safe_ollama_call(func, *args, **kwargs):
    """
    Ollama API 호출을 안전하게 실행합니다.
    서버가 실행되지 않은 경우 자동으로 시작을 시도합니다.
    
    Args:
        func: 실행할 ollama 함수
        *args, **kwargs: 함수에 전달할 인자들
        
    Returns:
        함수 실행 결과 또는 None (실패 시)
        
    Raises:
        Exception: 서버 시작 실패 또는 API 호출 실패 시
    """
    # 서버 상태 확인 및 필요시 시작
    server_ok, server_msg = ensure_ollama_server()
    if not server_ok:
        raise Exception(f"Ollama 서버를 사용할 수 없습니다: {server_msg}")
    
    try:
        return func(*args, **kwargs)
    except Exception as e:
        # 연결 오류인 경우 서버 재시작 시도
        if "connection" in str(e).lower() or "connect" in str(e).lower():
            logging.warning("Ollama 연결 오류 감지, 서버 재시작을 시도합니다...")
            start_success, start_msg = start_ollama_server()
            if start_success:
                # 재시작 후 한 번 더 시도
                return func(*args, **kwargs)
            else:
                raise Exception(f"Ollama 서버 재시작 실패: {start_msg}")
        else:
            raise e