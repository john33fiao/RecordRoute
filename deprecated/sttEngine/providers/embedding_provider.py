from __future__ import annotations

from abc import ABC, abstractmethod
from typing import Sequence

import numpy as np


class BaseEmbeddingProvider(ABC):
    @abstractmethod
    def embed(self, text: str, *, model: str) -> np.ndarray:
        raise NotImplementedError

    @abstractmethod
    def embed_batch(self, texts: Sequence[str], *, model: str) -> list[np.ndarray]:
        raise NotImplementedError

    @abstractmethod
    def healthcheck(self) -> tuple[bool, str]:
        raise NotImplementedError
