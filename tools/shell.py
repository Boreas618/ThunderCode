import os
import subprocess
import sys
import shutil
from typing import Optional

from tools.base import Tool


class ShellTool(Tool):
    """
    Tool for running shell commands via execvp() (Unix) or CreateProcessW() (Windows).
    
    The command is passed as an array of strings, with the first element being
    the program to execute and the rest being arguments.
    """
    
    # Store active PTY sessions for exec_command/write_stdin
    _sessions: dict = {}
    _next_session_id: int = 1
    
    @property
    def name(self) -> str:
        return "shell"
    
    @property
    def description(self) -> str:
        if sys.platform == "win32":
            return (
                "Runs a Powershell command (Windows) and returns its output. "
                "Arguments to `shell` will be passed to CreateProcessW(). "
                'Most commands should be prefixed with ["powershell.exe", "-Command"].'
            )
        else:
            return (
                "Runs a shell command and returns its output. "
                "The arguments to `shell` will be passed to execvp(). "
                'Most terminal commands should be prefixed with ["bash", "-lc"]. '
                "Always set the `workdir` param when using the shell function. "
                "Do not use `cd` unless absolutely necessary."
            )
    
    @property
    def parameters(self) -> dict:
        params = {
            "type": "object",
            "properties": {
                "command": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "The command to execute as an array of strings"
                },
                "workdir": {
                    "type": "string",
                    "description": "The working directory to execute the command in"
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "The timeout for the command in milliseconds"
                },
                "sandbox_permissions": {
                    "type": "string",
                    "description": (
                        "Sandbox permissions for the command. "
                        'Set to "require_escalated" to request running without sandbox restrictions; '
                        'defaults to "use_default".'
                    )
                },
                "justification": {
                    "type": "string",
                    "description": (
                        'Only set if sandbox_permissions is "require_escalated". '
                        "Request approval from the user to run this command outside the sandbox."
                    )
                },
                "prefix_rule": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": (
                        "Only specify when sandbox_permissions is `require_escalated`. "
                        "Suggest a prefix command pattern that will allow you to fulfill "
                        "similar requests from the user in the future."
                    )
                }
            },
            "required": ["command"],
            "additionalProperties": False
        }
        return params
    
    def execute(
        self,
        command: list[str],
        workdir: Optional[str] = None,
        timeout_ms: Optional[int] = None,
        sandbox_permissions: Optional[str] = None,
        justification: Optional[str] = None,
        prefix_rule: Optional[list[str]] = None,
    ) -> dict:
        if not command:
            return {"success": False, "error": "Command array cannot be empty"}
        
        try:
            # Set working directory
            cwd = workdir if workdir else os.getcwd()
            if not os.path.isdir(cwd):
                return {"success": False, "error": f"Working directory does not exist: {cwd}"}
            
            # Convert timeout
            timeout = timeout_ms / 1000.0 if timeout_ms else None
            
            # Execute command
            result = subprocess.run(
                command,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout
            )
            
            output = result.stdout
            if result.stderr:
                output += "\n" + result.stderr if output else result.stderr
            
            return {
                "success": result.returncode == 0,
                "exit_code": result.returncode,
                "output": output.strip() if output else ""
            }
            
        except subprocess.TimeoutExpired:
            return {"success": False, "error": f"Command timed out after {timeout_ms}ms"}
        except FileNotFoundError:
            return {"success": False, "error": f"Command not found: {command[0]}"}
        except PermissionError:
            return {"success": False, "error": f"Permission denied: {command[0]}"}
        except Exception as e:
            return {"success": False, "error": str(e)}


class ShellCommandTool(Tool):
    """
    Tool for running shell commands as a single string in the user's default shell.
    """
    
    @property
    def name(self) -> str:
        return "shell_command"
    
    @property
    def description(self) -> str:
        if sys.platform == "win32":
            return "Runs a Powershell command (Windows) and returns its output."
        else:
            return (
                "Runs a shell command and returns its output. "
                "Always set the `workdir` param when using the shell_command function. "
                "Do not use `cd` unless absolutely necessary."
            )
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell script to execute in the user's default shell"
                },
                "workdir": {
                    "type": "string",
                    "description": "The working directory to execute the command in"
                },
                "login": {
                    "type": "boolean",
                    "description": "Whether to run the shell with login shell semantics. Defaults to true."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "The timeout for the command in milliseconds"
                },
                "sandbox_permissions": {
                    "type": "string",
                    "description": (
                        "Sandbox permissions for the command. "
                        'Set to "require_escalated" to request running without sandbox restrictions; '
                        'defaults to "use_default".'
                    )
                },
                "justification": {
                    "type": "string",
                    "description": (
                        'Only set if sandbox_permissions is "require_escalated". '
                        "Request approval from the user to run this command outside the sandbox."
                    )
                },
                "prefix_rule": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": (
                        "Only specify when sandbox_permissions is `require_escalated`. "
                        "Suggest a prefix command pattern that will allow you to fulfill "
                        "similar requests from the user in the future."
                    )
                }
            },
            "required": ["command"],
            "additionalProperties": False
        }
    
    def _get_default_shell(self) -> str:
        """Get the user's default shell."""
        if sys.platform == "win32":
            return "powershell.exe"
        else:
            return os.environ.get("SHELL", "/bin/sh")
    
    def execute(
        self,
        command: str,
        workdir: Optional[str] = None,
        login: bool = True,
        timeout_ms: Optional[int] = None,
        sandbox_permissions: Optional[str] = None,
        justification: Optional[str] = None,
        prefix_rule: Optional[list[str]] = None,
    ) -> dict:
        if not command:
            return {"success": False, "error": "Command cannot be empty"}
        
        try:
            # Set working directory
            cwd = workdir if workdir else os.getcwd()
            if not os.path.isdir(cwd):
                return {"success": False, "error": f"Working directory does not exist: {cwd}"}
            
            # Convert timeout
            timeout = timeout_ms / 1000.0 if timeout_ms else None
            
            # Build shell command
            shell = self._get_default_shell()
            if sys.platform == "win32":
                shell_cmd = [shell, "-Command", command]
            else:
                if login:
                    shell_cmd = [shell, "-l", "-c", command]
                else:
                    shell_cmd = [shell, "-c", command]
            
            # Execute command
            result = subprocess.run(
                shell_cmd,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout
            )
            
            output = result.stdout
            if result.stderr:
                output += "\n" + result.stderr if output else result.stderr
            
            return {
                "success": result.returncode == 0,
                "exit_code": result.returncode,
                "output": output.strip() if output else ""
            }
            
        except subprocess.TimeoutExpired:
            return {"success": False, "error": f"Command timed out after {timeout_ms}ms"}
        except FileNotFoundError:
            return {"success": False, "error": f"Shell not found: {shell}"}
        except Exception as e:
            return {"success": False, "error": str(e)}


class ExecCommandTool(Tool):
    """
    Tool for running a command in a PTY, returning output or a session ID for ongoing interaction.
    """
    
    # Class-level session storage
    _sessions: dict = {}
    _next_session_id: int = 1
    
    @property
    def name(self) -> str:
        return "exec_command"
    
    @property
    def description(self) -> str:
        return "Runs a command in a PTY, returning output or a session ID for ongoing interaction."
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "cmd": {
                    "type": "string",
                    "description": "Shell command to execute."
                },
                "workdir": {
                    "type": "string",
                    "description": "Optional working directory to run the command in; defaults to the turn cwd."
                },
                "shell": {
                    "type": "string",
                    "description": "Shell binary to launch. Defaults to the user's default shell."
                },
                "login": {
                    "type": "boolean",
                    "description": "Whether to run the shell with -l/-i semantics. Defaults to true."
                },
                "tty": {
                    "type": "boolean",
                    "description": (
                        "Whether to allocate a TTY for the command. "
                        "Defaults to false (plain pipes); set to true to open a PTY and access TTY process."
                    )
                },
                "yield_time_ms": {
                    "type": "integer",
                    "description": "How long to wait (in milliseconds) for output before yielding."
                },
                "max_output_tokens": {
                    "type": "integer",
                    "description": "Maximum number of tokens to return. Excess output will be truncated."
                },
                "sandbox_permissions": {
                    "type": "string",
                    "description": (
                        "Sandbox permissions for the command. "
                        'Set to "require_escalated" to request running without sandbox restrictions; '
                        'defaults to "use_default".'
                    )
                },
                "justification": {
                    "type": "string",
                    "description": (
                        'Only set if sandbox_permissions is "require_escalated". '
                        "Request approval from the user to run this command outside the sandbox."
                    )
                },
                "prefix_rule": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": (
                        "Only specify when sandbox_permissions is `require_escalated`. "
                        "Suggest a prefix command pattern that will allow you to fulfill "
                        "similar requests from the user in the future."
                    )
                }
            },
            "required": ["cmd"],
            "additionalProperties": False
        }
    
    def _get_default_shell(self) -> str:
        """Get the user's default shell."""
        if sys.platform == "win32":
            return "powershell.exe"
        else:
            return os.environ.get("SHELL", "/bin/sh")
    
    def execute(
        self,
        cmd: str,
        workdir: Optional[str] = None,
        shell: Optional[str] = None,
        login: bool = True,
        tty: bool = False,
        yield_time_ms: Optional[int] = None,
        max_output_tokens: Optional[int] = None,
        sandbox_permissions: Optional[str] = None,
        justification: Optional[str] = None,
        prefix_rule: Optional[list[str]] = None,
    ) -> dict:
        if not cmd:
            return {"success": False, "error": "Command cannot be empty"}
        
        try:
            # Set working directory
            cwd = workdir if workdir else os.getcwd()
            if not os.path.isdir(cwd):
                return {"success": False, "error": f"Working directory does not exist: {cwd}"}
            
            # Determine shell
            shell_bin = shell if shell else self._get_default_shell()
            
            # Build shell command
            if sys.platform == "win32":
                shell_cmd = [shell_bin, "-Command", cmd]
            else:
                if login:
                    shell_cmd = [shell_bin, "-l", "-c", cmd]
                else:
                    shell_cmd = [shell_bin, "-c", cmd]
            
            # Convert timeout
            timeout = yield_time_ms / 1000.0 if yield_time_ms else None
            
            if tty:
                # For TTY mode, we need to create a session for interactive use
                # This is a simplified implementation - full PTY support would require pty module
                try:
                    import pty
                    import select
                    
                    master_fd, slave_fd = pty.openpty()
                    process = subprocess.Popen(
                        shell_cmd,
                        stdin=slave_fd,
                        stdout=slave_fd,
                        stderr=slave_fd,
                        cwd=cwd,
                        close_fds=True
                    )
                    os.close(slave_fd)
                    
                    # Store session
                    session_id = ExecCommandTool._next_session_id
                    ExecCommandTool._next_session_id += 1
                    ExecCommandTool._sessions[session_id] = {
                        "process": process,
                        "master_fd": master_fd,
                        "output_buffer": ""
                    }
                    
                    # Read initial output
                    output = ""
                    if yield_time_ms:
                        timeout_sec = yield_time_ms / 1000.0
                        ready, _, _ = select.select([master_fd], [], [], timeout_sec)
                        if ready:
                            output = os.read(master_fd, 65536).decode("utf-8", errors="replace")
                    
                    return {
                        "success": True,
                        "session_id": session_id,
                        "output": output,
                        "running": process.poll() is None
                    }
                    
                except ImportError:
                    # PTY not available on Windows
                    return {"success": False, "error": "TTY mode not available on this platform"}
            else:
                # Non-TTY mode: run command and capture output
                result = subprocess.run(
                    shell_cmd,
                    cwd=cwd,
                    capture_output=True,
                    text=True,
                    timeout=timeout
                )
                
                output = result.stdout
                if result.stderr:
                    output += "\n" + result.stderr if output else result.stderr
                
                # Truncate if max_output_tokens is set (rough approximation: 4 chars per token)
                if max_output_tokens and output:
                    max_chars = max_output_tokens * 4
                    if len(output) > max_chars:
                        output = output[:max_chars] + "\n... (output truncated)"
                
                return {
                    "success": result.returncode == 0,
                    "exit_code": result.returncode,
                    "output": output.strip() if output else ""
                }
                
        except subprocess.TimeoutExpired:
            return {"success": False, "error": f"Command timed out after {yield_time_ms}ms"}
        except FileNotFoundError:
            return {"success": False, "error": f"Shell not found: {shell_bin}"}
        except Exception as e:
            return {"success": False, "error": str(e)}


class WriteStdinTool(Tool):
    """
    Tool for writing characters to an existing unified exec session and returning recent output.
    """
    
    @property
    def name(self) -> str:
        return "write_stdin"
    
    @property
    def description(self) -> str:
        return "Writes characters to an existing unified exec session and returns recent output."
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "integer",
                    "description": "Identifier of the running unified exec session."
                },
                "chars": {
                    "type": "string",
                    "description": "Bytes to write to stdin (may be empty to poll)."
                },
                "yield_time_ms": {
                    "type": "integer",
                    "description": "How long to wait (in milliseconds) for output before yielding."
                },
                "max_output_tokens": {
                    "type": "integer",
                    "description": "Maximum number of tokens to return. Excess output will be truncated."
                }
            },
            "required": ["session_id"],
            "additionalProperties": False
        }
    
    def execute(
        self,
        session_id: int,
        chars: Optional[str] = None,
        yield_time_ms: Optional[int] = None,
        max_output_tokens: Optional[int] = None,
    ) -> dict:
        # Check if session exists
        if session_id not in ExecCommandTool._sessions:
            return {"success": False, "error": f"Session {session_id} not found"}
        
        session = ExecCommandTool._sessions[session_id]
        process = session["process"]
        master_fd = session["master_fd"]
        
        try:
            import select
            
            # Write input if provided
            if chars:
                os.write(master_fd, chars.encode("utf-8"))
            
            # Read output
            output = ""
            timeout_sec = (yield_time_ms / 1000.0) if yield_time_ms else 0.1
            
            ready, _, _ = select.select([master_fd], [], [], timeout_sec)
            if ready:
                output = os.read(master_fd, 65536).decode("utf-8", errors="replace")
            
            # Truncate if max_output_tokens is set
            if max_output_tokens and output:
                max_chars = max_output_tokens * 4
                if len(output) > max_chars:
                    output = output[:max_chars] + "\n... (output truncated)"
            
            # Check if process is still running
            running = process.poll() is None
            
            # Clean up if process has finished
            if not running:
                os.close(master_fd)
                del ExecCommandTool._sessions[session_id]
            
            return {
                "success": True,
                "output": output,
                "running": running,
                "exit_code": process.returncode if not running else None
            }
            
        except Exception as e:
            return {"success": False, "error": str(e)}
