from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Dict, Optional


class ProviderError(RuntimeError):
    """Base class for provider related failures."""


class ProviderConfigurationError(ProviderError):
    """Raised when provider configuration is invalid."""


class ProviderRequestError(ProviderError):
    """Raised when provider execution fails."""


@dataclass
class ProviderResolution:
    provider_type: str
    name: str
    options: Dict[str, Any]


def normalize_provider_name(
    provider_name: Optional[str],
    *,
    kind: str,
    default: str,
    aliases: Optional[Dict[str, str]] = None,
    supported: Optional[set[str]] = None,
) -> str:
    raw = (provider_name or default).strip().lower()
    normalized = (aliases or {}).get(raw, raw)
    if supported and normalized not in supported:
        raise ProviderConfigurationError(
            f"지원하지 않는 {kind} provider 입니다: {provider_name or raw}"
        )
    return normalized
