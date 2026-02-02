from abc import ABC, abstractmethod


class Tool(ABC):
    """Base class for all tools."""
    
    @property
    @abstractmethod
    def name(self) -> str:
        pass
    
    @property
    @abstractmethod
    def description(self) -> str:
        pass
    
    @property
    @abstractmethod
    def parameters(self) -> dict:
        pass
    
    @abstractmethod
    def execute(self, **kwargs) -> dict:
        pass
    
    def to_openai_tool(self) -> dict:
        return {
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.parameters
            }
        }


class ToolKit:
    """
    Manages a collection of tools for use with OpenAI chat completions API.
    
    Usage:
        toolkit = ToolKit()
        toolkit.register(SomeTool())
        
        # Get tools for API call
        tools = toolkit.get_openai_tools()
        
        # Execute a tool call
        result = toolkit.execute("tool_name", arg="value")
    """
    
    def __init__(self) -> None:
        self._tools: dict[str, Tool] = {}
    
    def register(self, tool: Tool) -> "ToolKit":
        self._tools[tool.name] = tool
        return self
    
    def get_tool(self, name: str) -> Tool | None:
        return self._tools.get(name)
    
    def get_openai_tools(self) -> list[dict]:
        return [tool.to_openai_tool() for tool in self._tools.values()]
    
    def execute(self, name: str, **kwargs) -> dict:
        tool = self._tools.get(name)
        if tool is None:
            return {"success": False, "error": f"Unknown tool: {name}"}
        return tool.execute(**kwargs)
    
    def __len__(self) -> int:
        return len(self._tools)
    
    def __contains__(self, name: str) -> bool:
        return name in self._tools
