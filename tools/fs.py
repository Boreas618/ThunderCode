import os
import re
import fnmatch
from pathlib import Path
from typing import Optional

from tools.base import Tool


class ReadFileTool(Tool):
    """
    Tool for reading a local file with 1-indexed line numbers.
    
    Supports two modes:
    - slice: Simple line ranges with offset and limit
    - indentation: Expands around an anchor line based on indentation levels
    """
    
    @property
    def name(self) -> str:
        return "read_file"
    
    @property
    def description(self) -> str:
        return "Reads a local file with 1-indexed line numbers, supporting slice and indentation-aware block modes."
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file."
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from. Must be 1 or greater."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to return."
                },
                "mode": {
                    "type": "string",
                    "description": 'Mode selector: "slice" for simple ranges (default) or "indentation" to expand around an anchor line.'
                },
                "indentation": {
                    "type": "object",
                    "description": "Indentation mode options.",
                    "properties": {
                        "anchor_line": {
                            "type": "integer",
                            "description": "Anchor line to center the indentation lookup on (defaults to offset)."
                        },
                        "max_levels": {
                            "type": "integer",
                            "description": "How many parent indentation levels (smaller indents) to include."
                        },
                        "include_siblings": {
                            "type": "boolean",
                            "description": "When true, include additional blocks that share the anchor indentation."
                        },
                        "include_header": {
                            "type": "boolean",
                            "description": "Include doc comments or attributes directly above the selected block."
                        },
                        "max_lines": {
                            "type": "integer",
                            "description": "Hard cap on the number of lines returned when using indentation mode."
                        }
                    }
                }
            },
            "required": ["file_path"],
            "additionalProperties": False
        }
    
    def _get_indentation(self, line: str) -> int:
        """Get the indentation level of a line (number of leading spaces)."""
        return len(line) - len(line.lstrip())
    
    def _read_slice_mode(
        self,
        lines: list[str],
        offset: int,
        limit: Optional[int]
    ) -> list[tuple[int, str]]:
        """Read lines in slice mode."""
        start_idx = offset - 1  # Convert to 0-indexed
        if limit:
            end_idx = start_idx + limit
        else:
            end_idx = len(lines)
        
        result = []
        for i in range(start_idx, min(end_idx, len(lines))):
            result.append((i + 1, lines[i]))  # Convert back to 1-indexed
        return result
    
    def _read_indentation_mode(
        self,
        lines: list[str],
        anchor_line: int,
        max_levels: Optional[int],
        include_siblings: bool,
        include_header: bool,
        max_lines: Optional[int]
    ) -> list[tuple[int, str]]:
        """Read lines in indentation mode, expanding around the anchor."""
        if anchor_line < 1 or anchor_line > len(lines):
            return []
        
        anchor_idx = anchor_line - 1  # Convert to 0-indexed
        anchor_indent = self._get_indentation(lines[anchor_idx])
        
        # Find the start of the block
        start_idx = anchor_idx
        levels_found = 0
        
        for i in range(anchor_idx - 1, -1, -1):
            line = lines[i]
            if not line.strip():  # Skip empty lines
                continue
            
            line_indent = self._get_indentation(line)
            
            if line_indent < anchor_indent:
                # Found a parent level
                levels_found += 1
                anchor_indent = line_indent
                start_idx = i
                
                if max_levels and levels_found >= max_levels:
                    break
            elif include_siblings and line_indent == anchor_indent:
                start_idx = i
        
        # Include header (doc comments, attributes) if requested
        if include_header:
            for i in range(start_idx - 1, -1, -1):
                line = lines[i].strip()
                if line.startswith("///") or line.startswith("#[") or line.startswith("//!"):
                    start_idx = i
                elif not line:
                    continue
                else:
                    break
        
        # Find the end of the block
        end_idx = anchor_idx
        block_indent = self._get_indentation(lines[start_idx])
        
        for i in range(anchor_idx + 1, len(lines)):
            line = lines[i]
            if not line.strip():  # Include empty lines within the block
                end_idx = i
                continue
            
            line_indent = self._get_indentation(line)
            
            if line_indent <= block_indent and line.strip():
                # End of block (or sibling block)
                if include_siblings and line_indent == block_indent:
                    end_idx = i
                else:
                    break
            else:
                end_idx = i
        
        # Apply max_lines limit
        result = []
        for i in range(start_idx, end_idx + 1):
            result.append((i + 1, lines[i]))  # Convert to 1-indexed
            if max_lines and len(result) >= max_lines:
                break
        
        return result
    
    def execute(
        self,
        file_path: str,
        offset: int = 1,
        limit: Optional[int] = None,
        mode: str = "slice",
        indentation: Optional[dict] = None,
    ) -> dict:
        if not file_path:
            return {"success": False, "error": "file_path is required"}
        
        if not os.path.isabs(file_path):
            return {"success": False, "error": f"file_path must be an absolute path: {file_path}"}
        
        if not os.path.exists(file_path):
            return {"success": False, "error": f"File not found: {file_path}"}
        
        if not os.path.isfile(file_path):
            return {"success": False, "error": f"Path is not a file: {file_path}"}
        
        if offset < 1:
            return {"success": False, "error": "offset must be 1 or greater"}
        
        try:
            with open(file_path, "r", encoding="utf-8", errors="replace") as f:
                lines = f.read().splitlines()
            
            total_lines = len(lines)
            
            if mode == "indentation" and indentation:
                anchor_line = indentation.get("anchor_line", offset)
                max_levels = indentation.get("max_levels")
                include_siblings = indentation.get("include_siblings", False)
                include_header = indentation.get("include_header", False)
                max_lines = indentation.get("max_lines")
                
                result_lines = self._read_indentation_mode(
                    lines, anchor_line, max_levels, include_siblings, include_header, max_lines
                )
            else:
                result_lines = self._read_slice_mode(lines, offset, limit)
            
            # Format output with line numbers
            output_lines = []
            for line_num, line_content in result_lines:
                output_lines.append(f"{line_num:6d}|{line_content}")
            
            return {
                "success": True,
                "content": "\n".join(output_lines),
                "total_lines": total_lines,
                "lines_returned": len(result_lines)
            }
            
        except PermissionError:
            return {"success": False, "error": f"Permission denied: {file_path}"}
        except Exception as e:
            return {"success": False, "error": str(e)}


class ListDirTool(Tool):
    """
    Tool for listing entries in a local directory with 1-indexed entry numbers.
    """
    
    @property
    def name(self) -> str:
        return "list_dir"
    
    @property
    def description(self) -> str:
        return "Lists entries in a local directory with 1-indexed entry numbers and simple type labels."
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "dir_path": {
                    "type": "string",
                    "description": "Absolute path to the directory to list."
                },
                "offset": {
                    "type": "integer",
                    "description": "Entry number to start listing from. Must be 1 or greater."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of entries to return."
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum directory depth to traverse. Must be 1 or greater."
                }
            },
            "required": ["dir_path"],
            "additionalProperties": False
        }
    
    def _get_entry_type(self, path: Path) -> str:
        """Get a simple type label for a path."""
        if path.is_dir():
            return "dir"
        elif path.is_symlink():
            return "link"
        elif path.is_file():
            return "file"
        else:
            return "other"
    
    def _list_entries(
        self,
        dir_path: Path,
        current_depth: int,
        max_depth: int,
        prefix: str = ""
    ) -> list[tuple[str, str]]:
        """Recursively list directory entries."""
        entries = []
        
        try:
            items = sorted(dir_path.iterdir(), key=lambda x: (not x.is_dir(), x.name.lower()))
        except PermissionError:
            return [(f"{prefix}(permission denied)", "error")]
        
        for item in items:
            # Skip hidden files
            if item.name.startswith("."):
                continue
            
            entry_type = self._get_entry_type(item)
            display_name = f"{prefix}{item.name}"
            
            if entry_type == "dir":
                display_name += "/"
            
            entries.append((display_name, entry_type))
            
            # Recurse into subdirectories if within depth limit
            if entry_type == "dir" and current_depth < max_depth:
                sub_entries = self._list_entries(
                    item,
                    current_depth + 1,
                    max_depth,
                    prefix + "  "
                )
                entries.extend(sub_entries)
        
        return entries
    
    def execute(
        self,
        dir_path: str,
        offset: int = 1,
        limit: Optional[int] = None,
        depth: int = 1,
    ) -> dict:
        if not dir_path:
            return {"success": False, "error": "dir_path is required"}
        
        if not os.path.isabs(dir_path):
            return {"success": False, "error": f"dir_path must be an absolute path: {dir_path}"}
        
        if not os.path.exists(dir_path):
            return {"success": False, "error": f"Directory not found: {dir_path}"}
        
        if not os.path.isdir(dir_path):
            return {"success": False, "error": f"Path is not a directory: {dir_path}"}
        
        if offset < 1:
            return {"success": False, "error": "offset must be 1 or greater"}
        
        if depth < 1:
            return {"success": False, "error": "depth must be 1 or greater"}
        
        try:
            path = Path(dir_path)
            all_entries = self._list_entries(path, 1, depth)
            
            total_entries = len(all_entries)
            
            # Apply offset and limit
            start_idx = offset - 1  # Convert to 0-indexed
            if limit:
                end_idx = start_idx + limit
            else:
                end_idx = len(all_entries)
            
            selected_entries = all_entries[start_idx:end_idx]
            
            # Format output with entry numbers
            output_lines = []
            for i, (name, entry_type) in enumerate(selected_entries, start=offset):
                output_lines.append(f"{i:6d}. [{entry_type:5s}] {name}")
            
            return {
                "success": True,
                "content": "\n".join(output_lines),
                "total_entries": total_entries,
                "entries_returned": len(selected_entries)
            }
            
        except PermissionError:
            return {"success": False, "error": f"Permission denied: {dir_path}"}
        except Exception as e:
            return {"success": False, "error": str(e)}


class GrepFilesTool(Tool):
    """
    Tool for finding files whose contents match a regex pattern.
    """
    
    @property
    def name(self) -> str:
        return "grep_files"
    
    @property
    def description(self) -> str:
        return "Finds files whose contents match the pattern and lists them by modification time."
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression pattern to search for."
                },
                "include": {
                    "type": "string",
                    "description": 'Glob that limits which files are searched (e.g., "*.rs" or "*.{ts,tsx}").'
                },
                "path": {
                    "type": "string",
                    "description": "Directory or file path to search. Defaults to the session's working directory."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of file paths to return (defaults to 100)."
                }
            },
            "required": ["pattern"],
            "additionalProperties": False
        }
    
    def _matches_glob(self, filepath: str, pattern: str) -> bool:
        """Check if a filepath matches a glob pattern."""
        # Handle patterns like "*.{ts,tsx}"
        if "{" in pattern and "}" in pattern:
            # Extract the base and extensions
            base, rest = pattern.split("{", 1)
            extensions, suffix = rest.split("}", 1)
            for ext in extensions.split(","):
                full_pattern = f"{base}{ext}{suffix}"
                if fnmatch.fnmatch(filepath, full_pattern):
                    return True
            return False
        return fnmatch.fnmatch(filepath, pattern)
    
    def _search_file(self, filepath: str, regex: re.Pattern) -> list[tuple[int, str]]:
        """Search a file for matches and return (line_number, line) tuples."""
        matches = []
        try:
            with open(filepath, "r", encoding="utf-8", errors="replace") as f:
                for i, line in enumerate(f, start=1):
                    if regex.search(line):
                        matches.append((i, line.rstrip()))
        except (PermissionError, OSError):
            pass
        return matches
    
    def execute(
        self,
        pattern: str,
        include: Optional[str] = None,
        path: Optional[str] = None,
        limit: int = 100,
    ) -> dict:
        if not pattern:
            return {"success": False, "error": "pattern is required"}
        
        try:
            regex = re.compile(pattern)
        except re.error as e:
            return {"success": False, "error": f"Invalid regex pattern: {e}"}
        
        search_path = path if path else os.getcwd()
        
        if not os.path.exists(search_path):
            return {"success": False, "error": f"Path not found: {search_path}"}
        
        try:
            # Collect matching files with their modification times
            matching_files = []
            
            if os.path.isfile(search_path):
                # Search single file
                matches = self._search_file(search_path, regex)
                if matches:
                    mtime = os.path.getmtime(search_path)
                    matching_files.append((search_path, mtime, matches))
            else:
                # Search directory recursively
                for root, dirs, files in os.walk(search_path):
                    # Skip hidden directories
                    dirs[:] = [d for d in dirs if not d.startswith(".")]
                    
                    for filename in files:
                        # Skip hidden files
                        if filename.startswith("."):
                            continue
                        
                        filepath = os.path.join(root, filename)
                        
                        # Apply glob filter
                        if include and not self._matches_glob(filename, include):
                            continue
                        
                        matches = self._search_file(filepath, regex)
                        if matches:
                            try:
                                mtime = os.path.getmtime(filepath)
                                matching_files.append((filepath, mtime, matches))
                            except OSError:
                                continue
                        
                        # Check limit
                        if len(matching_files) >= limit:
                            break
                    
                    if len(matching_files) >= limit:
                        break
            
            # Sort by modification time (most recent first)
            matching_files.sort(key=lambda x: x[1], reverse=True)
            
            # Apply limit
            matching_files = matching_files[:limit]
            
            # Format output
            output_lines = []
            for filepath, mtime, matches in matching_files:
                rel_path = os.path.relpath(filepath, search_path) if os.path.isdir(search_path) else filepath
                output_lines.append(f"\n{rel_path}:")
                for line_num, line_content in matches[:10]:  # Limit matches per file
                    # Truncate long lines
                    if len(line_content) > 200:
                        line_content = line_content[:200] + "..."
                    output_lines.append(f"  {line_num}: {line_content}")
                if len(matches) > 10:
                    output_lines.append(f"  ... ({len(matches) - 10} more matches)")
            
            return {
                "success": True,
                "content": "\n".join(output_lines).strip(),
                "files_matched": len(matching_files),
                "limit_applied": len(matching_files) >= limit
            }
            
        except PermissionError:
            return {"success": False, "error": f"Permission denied: {search_path}"}
        except Exception as e:
            return {"success": False, "error": str(e)}
