# llamacpp_utils.py
"""
llama.cpp 기반 로컬 LLM 엔진 유틸리티
Ollama 대신 GGUF 모델 파일을 직접 로드하여 self-contained 실행 지원
"""

import logging
import os
import platform
from pathlib import Path
from typing import Optional, Tuple, Iterator
import threading

try:
    from llama_cpp import Llama
except ImportError:
    Llama = None

# 모델 디렉토리 설정
MODELS_DIR = Path(__file__).parent.parent / "models"
MODELS_DIR.mkdir(exist_ok=True)

# 전역 모델 인스턴스 캐시 (싱글톤 패턴)
_model_cache = {}
_model_lock = threading.Lock()


def get_model_path(model_filename: str) -> Optional[Path]:
    """
    모델 파일의 절대 경로를 반환합니다.

    Args:
        model_filename: GGUF 모델 파일명 (예: "gemma-3-4b-it-Q4_K_M.gguf")

    Returns:
        Path: 모델 파일 경로 (존재하지 않으면 None)
    """
    model_path = MODELS_DIR / model_filename

    # 파일 존재 확인
    if model_path.exists():
        return model_path

    # 확장자 없이 제공된 경우 .gguf 추가
    if not model_filename.endswith('.gguf'):
        model_path_with_ext = MODELS_DIR / f"{model_filename}.gguf"
        if model_path_with_ext.exists():
            return model_path_with_ext

    return None


def check_model_available(model_filename: str) -> Tuple[bool, str]:
    """
    지정된 GGUF 모델 파일이 사용 가능한지 확인합니다.

    Args:
        model_filename: 확인할 모델 파일명

    Returns:
        Tuple[bool, str]: (모델 사용 가능 여부, 메시지)
    """
    if Llama is None:
        return False, "llama-cpp-python 패키지가 설치되지 않았습니다."

    model_path = get_model_path(model_filename)

    if model_path is None:
        return False, f"모델 파일 '{model_filename}'을 찾을 수 없습니다. 경로: {MODELS_DIR}"

    # 파일 크기 확인
    file_size_mb = model_path.stat().st_size / (1024 * 1024)
    return True, f"모델 '{model_filename}'이 사용 가능합니다. (크기: {file_size_mb:.1f}MB)"


def get_default_gpu_layers() -> int:
    """
    플랫폼에 따라 기본 GPU 레이어 수를 반환합니다.

    Returns:
        int: GPU 레이어 수 (-1은 전체 GPU 오프로드, 0은 CPU 전용)
    """
    system = platform.system()

    # macOS (Apple Silicon) - Metal 지원
    if system == "Darwin":
        # M1/M2/M3 칩의 경우 Metal 사용
        return -1  # 전체 GPU 오프로드

    # Windows/Linux - CUDA/ROCm 확인 (기본적으로 GPU 사용 시도)
    # llama.cpp는 자동으로 사용 가능한 GPU 백엔드를 감지
    return -1  # 전체 GPU 오프로드 시도 (없으면 자동으로 CPU로 폴백)


def load_model(
    model_filename: str,
    n_ctx: int = 4096,
    n_gpu_layers: Optional[int] = None,
    verbose: bool = False
) -> Llama:
    """
    GGUF 모델을 로드합니다. 싱글톤 패턴으로 이미 로드된 모델은 재사용합니다.

    Args:
        model_filename: 로드할 GGUF 모델 파일명
        n_ctx: 컨텍스트 윈도우 크기 (토큰 수)
        n_gpu_layers: GPU에 오프로드할 레이어 수 (-1: 전체, 0: CPU 전용, None: 자동)
        verbose: 상세 로그 출력 여부

    Returns:
        Llama: 로드된 모델 인스턴스

    Raises:
        FileNotFoundError: 모델 파일을 찾을 수 없는 경우
        Exception: 모델 로딩 실패 시
    """
    if Llama is None:
        raise ImportError(
            "llama-cpp-python이 설치되지 않았습니다. "
            "'pip install llama-cpp-python' 명령어로 설치하세요."
        )

    # 모델 경로 확인
    model_path = get_model_path(model_filename)
    if model_path is None:
        raise FileNotFoundError(
            f"모델 파일 '{model_filename}'을 찾을 수 없습니다. "
            f"모델 디렉토리: {MODELS_DIR}"
        )

    # GPU 레이어 수 자동 설정
    if n_gpu_layers is None:
        n_gpu_layers = get_default_gpu_layers()

    # 캐시 키 생성 (모델명 + 설정)
    cache_key = f"{model_filename}_{n_ctx}_{n_gpu_layers}"

    # 캐시된 모델이 있으면 재사용
    with _model_lock:
        if cache_key in _model_cache:
            logging.info(f"캐시된 모델 '{model_filename}' 재사용")
            return _model_cache[cache_key]

        # 새로운 모델 로드
        logging.info(f"모델 '{model_filename}' 로딩 시작... (GPU 레이어: {n_gpu_layers}, 컨텍스트: {n_ctx})")

        try:
            model = Llama(
                model_path=str(model_path),
                n_ctx=n_ctx,
                n_gpu_layers=n_gpu_layers,
                verbose=verbose
            )

            # 캐시에 저장
            _model_cache[cache_key] = model

            logging.info(f"모델 '{model_filename}' 로딩 완료")
            return model

        except Exception as e:
            logging.error(f"모델 '{model_filename}' 로딩 실패: {str(e)}")
            raise Exception(f"모델 로딩 실패: {str(e)}")


def generate_text(
    model_filename: str,
    prompt: str,
    max_tokens: int = 512,
    temperature: float = 0.7,
    top_p: float = 0.9,
    top_k: int = 40,
    stop: Optional[list] = None,
    stream: bool = False,
    **kwargs
) -> str | Iterator[str]:
    """
    주어진 프롬프트로 텍스트를 생성합니다.

    Args:
        model_filename: 사용할 GGUF 모델 파일명
        prompt: 입력 프롬프트
        max_tokens: 생성할 최대 토큰 수
        temperature: 샘플링 온도 (0.0-2.0, 낮을수록 결정적)
        top_p: Nucleus sampling 파라미터
        top_k: Top-K sampling 파라미터
        stop: 생성 중단 토큰 목록
        stream: 스트리밍 모드 여부
        **kwargs: 추가 llama.cpp 파라미터

    Returns:
        str: 생성된 텍스트 (stream=False)
        Iterator[str]: 텍스트 스트림 (stream=True)
    """
    # 모델 로드
    model = load_model(model_filename, n_ctx=kwargs.get('n_ctx', 4096))

    # 기본 중단 토큰 설정
    if stop is None:
        stop = []

    try:
        # 텍스트 생성
        response = model(
            prompt,
            max_tokens=max_tokens,
            temperature=temperature,
            top_p=top_p,
            top_k=top_k,
            stop=stop,
            stream=stream,
            **kwargs
        )

        if stream:
            # 스트리밍 모드: Iterator 반환
            def stream_generator():
                for chunk in response:
                    if 'choices' in chunk and len(chunk['choices']) > 0:
                        text = chunk['choices'][0].get('text', '')
                        if text:
                            yield text
            return stream_generator()
        else:
            # 일반 모드: 완성된 텍스트 반환
            if 'choices' in response and len(response['choices']) > 0:
                return response['choices'][0]['text']
            else:
                raise Exception("모델 응답 형식이 올바르지 않습니다.")

    except Exception as e:
        logging.error(f"텍스트 생성 실패: {str(e)}")
        raise Exception(f"텍스트 생성 실패: {str(e)}")


def unload_model(model_filename: str = None):
    """
    로드된 모델을 메모리에서 해제합니다.

    Args:
        model_filename: 해제할 모델 파일명 (None이면 모든 모델 해제)
    """
    with _model_lock:
        if model_filename is None:
            # 모든 모델 해제
            count = len(_model_cache)
            _model_cache.clear()
            logging.info(f"{count}개 모델 메모리 해제 완료")
        else:
            # 특정 모델만 해제
            keys_to_remove = [k for k in _model_cache.keys() if k.startswith(model_filename)]
            for key in keys_to_remove:
                del _model_cache[key]
            logging.info(f"모델 '{model_filename}' 메모리 해제 완료")


# 하위 호환성을 위한 별칭
ensure_model_available = check_model_available
