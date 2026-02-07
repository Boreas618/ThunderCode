"""NanoCoder — AI coding agent library.

Accepts a prompt and processes it with an LLM agent backed by a configurable
toolkit.  Works as the core for both interactive CLI and offline/programmatic
usage.

Usage (async context manager)::

    from nano_coder import NanoCoder

    async with NanoCoder() as coder:
        response = await coder.prompt("Write hello world in Python")
        print(response)

Usage (manual lifecycle)::

    coder = NanoCoder(api_key="sk-...", model="gpt-4o")
    await coder.aopen()
    response = await coder.prompt("Explain quicksort")
    await coder.aclose()
"""

from __future__ import annotations

import os
from collections.abc import Sequence
from pathlib import Path
from typing import Any, Callable

from openai import AsyncOpenAI, DefaultAioHttpClient

from session import Session, ToolStartCallback, ToolResultCallback
from tools import ToolKit, create_default_toolkit

_DEFAULT_SYSTEM_PROMPT_PATH = Path(__file__).parent / "system_prompt.md"


class NanoCoder:
    """High-level agent that accepts prompts and processes them via tool-augmented LLM."""

    def __init__(
        self,
        *,
        api_key: str | None = None,
        base_url: str | None = None,
        model: str | None = None,
        system_prompt: str | None = None,
        tools: Sequence[str] | None = None,
        toolkit: ToolKit | None = None,
        timeout: int = 120,
        on_tool_start: ToolStartCallback | None = None,
        on_tool_result: ToolResultCallback | None = None,
    ) -> None:
        """
        Parameters
        ----------
        api_key : str, optional
            OpenAI-compatible API key.  Falls back to ``OPENAI_API_KEY`` env var.
        base_url : str, optional
            API base URL.  Falls back to ``OPENAI_BASE_URL`` env var.
        model : str, optional
            Model identifier.  Falls back to ``MODEL`` env var, then
            ``"aws/anthropic/claude-opus-4-5"``.
        system_prompt : str, optional
            System prompt text.  If not provided, loaded from the bundled
            ``system_prompt.md``.
        tools : sequence of str, optional
            Tool names and/or tool-set names to include.  Accepts any mix of
            individual names (e.g. ``"read_file"``) and set names
            (e.g. ``"readonly"``, ``"filesystem"``, ``"shell"``, ``"all"``).
            Ignored when *toolkit* is provided.  Defaults to all tools.

            Available sets:
              - ``"readonly"``   — read_file, list_dir, grep_files
              - ``"filesystem"`` — readonly + apply_patch
              - ``"shell"``      — shell, shell_command, exec_command, write_stdin
              - ``"patch"``      — apply_patch
              - ``"plan"``       — update_plan
              - ``"all"``        — every registered tool
        toolkit : ToolKit, optional
            A fully pre-configured ``ToolKit`` instance.  When provided,
            *tools* is ignored.  Use this for complete control over tool
            registration and configuration.
        timeout : int
            HTTP request timeout in seconds (default 120).
        on_tool_start : callable, optional
            ``(name, arguments) -> None`` — invoked before each tool execution.
        on_tool_result : callable, optional
            ``(name, arguments, result) -> None`` — invoked after each tool execution.
        """
        self.api_key = api_key or os.environ.get("OPENAI_API_KEY")
        self.base_url = base_url or os.environ.get("OPENAI_BASE_URL")
        self.model = model or os.environ.get("MODEL", "aws/anthropic/claude-opus-4-5")
        self.timeout = timeout
        self.on_tool_start = on_tool_start
        self.on_tool_result = on_tool_result

        if system_prompt is None:
            with open(_DEFAULT_SYSTEM_PROMPT_PATH) as f:
                system_prompt = f.read()
        self._system_prompt = system_prompt

        self._toolkit = toolkit or create_default_toolkit(tools=tools)
        self._session = self._make_session()
        self._client: AsyncOpenAI | None = None

    # -- Async context manager -------------------------------------------

    async def __aenter__(self) -> NanoCoder:
        return await self.aopen()

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.aclose()

    async def aopen(self) -> NanoCoder:
        """Open the underlying HTTP client.  Prefer ``async with NanoCoder()``."""
        self._client = AsyncOpenAI(
            api_key=self.api_key,
            base_url=self.base_url,
            http_client=DefaultAioHttpClient(),
            timeout=self.timeout,
        )
        return self

    async def aclose(self) -> None:
        """Close the underlying HTTP client and release resources."""
        if self._client is not None:
            await self._client.close()
            self._client = None

    # -- Public API ------------------------------------------------------

    async def prompt(self, text: str) -> str:
        """Submit a user prompt and return the agent's final text response.

        The agent loop runs to completion, executing any tool calls the model
        requests along the way.  Conversation history is preserved across
        calls so the agent has full context of prior turns.
        """
        if self._client is None:
            raise RuntimeError(
                "NanoCoder client is not initialized. "
                "Use 'async with NanoCoder() as coder:' or call aopen() first."
            )
        self._session.append_user(text)
        return await self._session(self._client, self.model)

    def reset(self) -> None:
        """Clear conversation history and start a fresh session."""
        self._session = self._make_session()

    # -- Properties ------------------------------------------------------

    @property
    def session(self) -> Session:
        """The underlying ``Session`` (for advanced / inspection use)."""
        return self._session

    # -- Internal --------------------------------------------------------

    def _make_session(self) -> Session:
        return Session(
            tool_kit=self._toolkit,
            system_prompt=self._system_prompt,
            on_tool_start=self.on_tool_start,
            on_tool_result=self.on_tool_result,
        )
