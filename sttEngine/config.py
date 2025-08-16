"""
RecordRoute 설정 관리 모듈
플랫폼별 모델 설정을 .env 파일에서 로드하고 관리합니다.
"""
import os
import platform
from pathlib import Path
from typing import Dict, Any

def load_env_file(env_path: Path = None) -> Dict[str, str]:
    """
    .env 파일을 로드하여 환경변수로 설정
    
    Args:
        env_path: .env 파일 경로 (기본값: 프로젝트 루트의 .env)
    
    Returns:
        로드된 환경변수 딕셔너리
    """
    if env_path is None:
        # 현재 파일의 2단계 상위 디렉토리 (프로젝트 루트)
        project_root = Path(__file__).parent.parent
        env_path = project_root / ".env"
    
    env_vars = {}
    
    if env_path.exists():
        with open(env_path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith('#') and '=' in line:
                    key, value = line.split('=', 1)
                    env_vars[key.strip()] = value.strip()
                    os.environ[key.strip()] = value.strip()
    
    return env_vars

def get_platform_suffix() -> str:
    """현재 플랫폼에 맞는 접미사 반환"""
    return "WINDOWS" if platform.system() == "Windows" else "UNIX"

def get_model_for_task(task: str, fallback: str = None) -> str:
    """
    작업별 플랫폼에 맞는 모델명 반환
    
    Args:
        task: 작업 유형 ("TRANSCRIBE", "SUMMARY")
        fallback: 환경변수가 없을 때 사용할 기본값
    
    Returns:
        모델명
    """
    platform_suffix = get_platform_suffix()
    env_key = f"{task}_MODEL_{platform_suffix}"
    
    model = os.getenv(env_key, fallback)
    if not model:
        raise ValueError(f"모델 설정을 찾을 수 없습니다: {env_key}")
    
    return model

def get_config_value(key: str, default: Any = None, value_type: type = str) -> Any:
    """
    환경변수에서 설정값을 가져와 지정된 타입으로 변환
    
    Args:
        key: 환경변수 키
        default: 기본값
        value_type: 반환할 타입 (str, int, float, bool)
    
    Returns:
        변환된 설정값
    """
    value = os.getenv(key, default)
    
    if value is None:
        return default
    
    if value_type == bool:
        return str(value).lower() in ('true', '1', 'yes', 'on')
    elif value_type == int:
        return int(value)
    elif value_type == float:
        return float(value)
    else:
        return str(value)

# 자동으로 .env 파일 로드
load_env_file()

# 플랫폼별 기본값 (환경변수가 없을 때 사용)
PLATFORM_DEFAULTS = {
    "TRANSCRIBE": {
        "WINDOWS": "large-v3-turbo",
        "UNIX": "large-v3-turbo"
    },
    "SUMMARY": {
        "WINDOWS": "gemma3:4b",
        "UNIX": "gpt-oss:20b"
    },
    "EMBEDDING": {
        "WINDOWS": "snowflake-arctic-embed2:latest",
        "UNIX": "snowflake-arctic-embed2:latest"
    }
}

def get_default_model(task: str) -> str:
    """작업별 플랫폼 기본 모델 반환"""
    platform_suffix = get_platform_suffix()
    return PLATFORM_DEFAULTS[task][platform_suffix]