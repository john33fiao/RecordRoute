"""
RecordRoute 설정 관리 모듈
플랫폼별 모델 설정을 .env 파일에서 로드하고 관리합니다.
"""
import os
import platform
import sys
from pathlib import Path
from typing import Any, Dict, Optional

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
        "WINDOWS": "bge-m3:latest",
        "UNIX": "bge-m3:latest"
    }
}

def get_default_model(task: str) -> str:
    """작업별 플랫폼 기본 모델 반환"""
    platform_suffix = get_platform_suffix()
    return PLATFORM_DEFAULTS[task][platform_suffix]


DB_ALIAS = "DB"
DEFAULT_DB_FOLDER = "/DB"


def get_project_root() -> Path:
    """프로젝트 루트 경로 반환"""
    return Path(getattr(sys, "_MEIPASS", Path(__file__).parent.parent)).resolve()


def _resolve_db_path(path_value: str, base_dir: Path) -> Optional[Path]:
    """주어진 문자열을 절대 경로로 변환"""
    if not path_value:
        return None

    try:
        candidate = Path(path_value)
        if not candidate.is_absolute():
            candidate = (base_dir / candidate).resolve()
        else:
            candidate = candidate.resolve()
        return candidate
    except Exception:
        return None


def _ensure_directory_accessible(path: Path, *, create_if_missing: bool = True) -> bool:
    """경로가 디렉터리로 접근 가능한지 확인

    Args:
        path: 접근성을 확인할 경로.
        create_if_missing: 경로가 존재하지 않을 때 디렉터리를 생성할지 여부.
    """
    try:
        if path.exists():
            if not path.is_dir():
                return False
        else:
            if create_if_missing:
                path.mkdir(parents=True, exist_ok=True)
            else:
                return False
    except Exception:
        return False

    return os.access(path, os.R_OK | os.W_OK | os.X_OK)


def get_db_base_path(base_dir: Optional[Path] = None) -> Path:
    """환경변수 기반 DB 폴더 경로 반환 (접근 불가 시 기본값 생성 후 사용)"""
    if base_dir is None:
        base_dir = get_project_root()

    env_value = os.getenv("DB_FOLDER_PATH")

    if env_value:
        env_path = _resolve_db_path(env_value, base_dir)
        if env_path and _ensure_directory_accessible(env_path, create_if_missing=False):
            return env_path

    default_path = _resolve_db_path(DEFAULT_DB_FOLDER, base_dir)
    if default_path and _ensure_directory_accessible(default_path):
        return default_path

    fallback = (base_dir / DB_ALIAS).resolve()
    fallback.mkdir(parents=True, exist_ok=True)
    return fallback


def normalize_db_record_path(path_str: str, base_dir: Optional[Path] = None) -> str:
    """DB 경로 문자열을 표준화하여 저장용 경로로 변환"""
    if not path_str:
        return ""

    normalized = path_str.replace("\\", "/").lstrip("/")
    if normalized.startswith(f"{DB_ALIAS}/"):
        return normalized

    db_root = get_db_base_path(base_dir)
    path_obj = Path(path_str)

    if path_obj.is_absolute():
        try:
            relative = path_obj.resolve().relative_to(db_root)
            return f"{DB_ALIAS}/{relative.as_posix()}"
        except ValueError:
            return normalized

    return f"{DB_ALIAS}/{normalized}"


def resolve_db_path(path_str: str, base_dir: Optional[Path] = None) -> Path:
    """저장된 DB 경로 문자열을 실제 경로로 변환"""
    if not path_str:
        raise ValueError("path_str must be a non-empty string")

    normalized = normalize_db_record_path(path_str, base_dir)
    if normalized.startswith(f"{DB_ALIAS}/"):
        relative = normalized[len(DB_ALIAS) + 1 :]
        db_root = get_db_base_path(base_dir)
        return (db_root / relative).resolve()

    return Path(normalized).resolve()


def to_db_record_path(path: Path, base_dir: Optional[Path] = None) -> str:
    """실제 경로를 저장용 DB 경로 문자열로 변환"""
    db_root = get_db_base_path(base_dir)
    try:
        relative = path.resolve().relative_to(db_root)
        return f"{DB_ALIAS}/{relative.as_posix()}"
    except ValueError:
        return path.resolve().as_posix()
