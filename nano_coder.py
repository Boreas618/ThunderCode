from openai import AsyncOpenAI, DefaultAioHttpClient
from dotenv import load_dotenv
from tools import create_default_toolkit
from session import Session
import os
import asyncio

load_dotenv()


async def main():
    with open("system_prompt.md") as f:
        system_prompt = f.read()

    toolkit = create_default_toolkit()
    session = Session(tool_kit=toolkit, system_prompt=system_prompt)

    async with AsyncOpenAI(
        api_key=os.environ.get("OPENAI_API_KEY"),
        base_url=os.environ.get("OPENAI_BASE_URL"),
        http_client=DefaultAioHttpClient(),
        timeout=120
    ) as client:
        model = os.environ.get("MODEL", "aws/anthropic/claude-opus-4-5")
        
        while True:
            try:
                user_input = input("\n>>> ")
            except (EOFError, KeyboardInterrupt):
                break

            if not user_input.strip():
                continue

            session.append_user(user_input)
            response = await session(client, model)
            print(f"\n{response}")


if __name__ == '__main__':
    asyncio.run(main())
