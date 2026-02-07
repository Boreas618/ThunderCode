from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Callable
from openai import AsyncOpenAI
from tools import ToolKit
import json

# Callback type for tool execution events.
# on_tool_start(name, arguments) — called before a tool executes.
# on_tool_result(name, arguments, result) — called after a tool executes.
ToolStartCallback = Callable[[str, dict[str, Any]], None]
ToolResultCallback = Callable[[str, dict[str, Any], dict[str, Any]], None]


class MessageRole(Enum):
    SYSTEM = "system"
    ASSISTANT = "assistant"
    USER = "user"
    TOOL = "tool"


@dataclass
class ToolCall:
    id: str
    name: str
    arguments: dict[str, Any]


@dataclass
class Message:
    role: MessageRole
    content: str | None = None
    tool_calls: list[ToolCall] = field(default_factory=list)
    tool_call_id: str | None = None  # For TOOL role messages


class Session:
    def __init__(
        self,
        tool_kit: ToolKit,
        system_prompt: str,
        on_tool_start: ToolStartCallback | None = None,
        on_tool_result: ToolResultCallback | None = None,
    ) -> None:
        self.tool_kit = tool_kit
        self.on_tool_start = on_tool_start
        self.on_tool_result = on_tool_result
        self.messages: list[Message] = [
            Message(role=MessageRole.SYSTEM, content=system_prompt)
        ]

    def append_user(self, content: str) -> None:
        self.messages.append(Message(role=MessageRole.USER, content=content))

    def append_assistant(self, message: Any) -> None:
        """Append assistant message from OpenAI response."""
        tool_calls = []
        if message.tool_calls:
            for tc in message.tool_calls:
                tool_calls.append(ToolCall(
                    id=tc.id,
                    name=tc.function.name,
                    arguments=json.loads(tc.function.arguments)
                ))

        self.messages.append(Message(
            role=MessageRole.ASSISTANT,
            content=message.content,
            tool_calls=tool_calls
        ))

    def append_tool(self, tool_call_id: str, result: dict[str, Any]) -> None:
        """Append tool execution result."""
        self.messages.append(Message(
            role=MessageRole.TOOL,
            content=json.dumps(result, ensure_ascii=False),
            tool_call_id=tool_call_id,
        ))

    def to_openai_messages(self) -> list[dict[str, Any]]:
        """Convert internal messages to OpenAI API format."""
        result = []
        for msg in self.messages:
            if msg.role == MessageRole.TOOL:
                # Tool result message
                result.append({
                    "role": "tool",
                    "tool_call_id": msg.tool_call_id,
                    "content": msg.content,
                })
            elif msg.role == MessageRole.ASSISTANT and msg.tool_calls:
                # Assistant message with tool calls
                result.append({
                    "role": "assistant",
                    "content": msg.content,
                    "tool_calls": [
                        {
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": json.dumps(tc.arguments, ensure_ascii=False),
                            }
                        }
                        for tc in msg.tool_calls
                    ],
                })
            else:
                # System, user, or assistant without tool calls
                result.append({
                    "role": msg.role.value,
                    "content": msg.content,
                })
        return result

    async def __call__(self, client: AsyncOpenAI, model: str) -> str:
        """
        Execute one turn of conversation with potential tool call loops.
        Returns the final text response from the assistant.
        """
        while True:
            completion = await client.chat.completions.create(
                model=model,
                messages=self.to_openai_messages(),
                tools=self.tool_kit.get_openai_tools() or None
            )

            message = completion.choices[0].message
            self.append_assistant(message)

            if not message.tool_calls:
                return message.content or ""

            # Execute all tool calls and add results to history
            for tool_call in message.tool_calls:
                args = json.loads(tool_call.function.arguments)
                if self.on_tool_start:
                    self.on_tool_start(tool_call.function.name, args)
                result = self.tool_kit.execute(tool_call.function.name, **args)
                self.append_tool(tool_call.id, result)
                if self.on_tool_result:
                    self.on_tool_result(tool_call.function.name, args, result)
