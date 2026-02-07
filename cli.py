"""NanoCoder CLI — interactive terminal interface."""

import asyncio
from typing import Any

from dotenv import load_dotenv

from nano_coder import NanoCoder


def _on_tool_start(name: str, arguments: dict[str, Any]) -> None:
    print(f"[Tool] {name}")


def _on_tool_result(name: str, arguments: dict[str, Any], result: dict[str, Any]) -> None:
    if result.get("success"):
        print(f"  ✓ {result.get('message', 'OK')}")
    else:
        print(f"  ✗ {result.get('error', 'Failed')}")


async def main() -> None:
    load_dotenv()

    async with NanoCoder(
        on_tool_start=_on_tool_start,
        on_tool_result=_on_tool_result,
    ) as coder:
        while True:
            try:
                user_input = input("\n>>> ")
            except (EOFError, KeyboardInterrupt):
                break

            if not user_input.strip():
                continue

            response = await coder.prompt(user_input)
            print(f"\n{response}")


if __name__ == "__main__":
    asyncio.run(main())
