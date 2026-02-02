import os

from tools.base import Tool


class ApplyPatchTool(Tool):
    """
    Tool for applying patches to files.
    
    Supported operations:
    - *** Add File: <path> - create a new file
    - *** Delete File: <path> - remove an existing file  
    - *** Update File: <path> - patch an existing file (with optional Move to:)
    """
    
    @property
    def name(self) -> str:
        return "apply_patch"
    
    @property
    def description(self) -> str:
        return "Modifies a file by applying a patch."
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "The patch content to apply"
                }
            },
            "required": ["patch"]
        }
    
    def execute(self, patch: str = "") -> dict:
        results = []
        lines = patch.strip().split('\n')
        
        # Validate envelope
        if not lines or lines[0].strip() != '*** Begin Patch':
            return {'success': False, 'error': 'Patch must start with "*** Begin Patch"'}
        if lines[-1].strip() != '*** End Patch':
            return {'success': False, 'error': 'Patch must end with "*** End Patch"'}
        
        # Parse file sections
        lines = lines[1:-1]  # Remove envelope
        i = 0
        
        while i < len(lines):
            line = lines[i].strip()
            
            if not line:
                i += 1
                continue
                
            # Add File
            if line.startswith('*** Add File:'):
                filepath = line[len('*** Add File:'):].strip()
                i += 1
                content_lines = []
                while i < len(lines) and not lines[i].strip().startswith('***'):
                    content_line = lines[i]
                    if content_line.startswith('+'):
                        content_lines.append(content_line[1:])  # Remove + prefix
                    i += 1
                try:
                    if os.path.dirname(filepath):
                        os.makedirs(os.path.dirname(filepath), exist_ok=True)
                    with open(filepath, 'w') as f:
                        f.write('\n'.join(content_lines))
                    results.append(f'Added file: {filepath}')
                except Exception as e:
                    return {'success': False, 'error': f'Failed to add file {filepath}: {e}'}
                    
            # Delete File
            elif line.startswith('*** Delete File:'):
                filepath = line[len('*** Delete File:'):].strip()
                i += 1
                try:
                    if os.path.exists(filepath):
                        os.remove(filepath)
                        results.append(f'Deleted file: {filepath}')
                    else:
                        results.append(f'File already deleted or not found: {filepath}')
                except Exception as e:
                    return {'success': False, 'error': f'Failed to delete file {filepath}: {e}'}
                    
            # Update File
            elif line.startswith('*** Update File:'):
                filepath = line[len('*** Update File:'):].strip()
                i += 1
                new_filepath = None
                
                # Check for Move to:
                if i < len(lines) and lines[i].strip().startswith('*** Move to:'):
                    new_filepath = lines[i].strip()[len('*** Move to:'):].strip()
                    i += 1
                
                # Read existing file
                try:
                    with open(filepath, 'r') as f:
                        file_content = f.read()
                    file_lines = file_content.split('\n')
                except Exception as e:
                    return {'success': False, 'error': f'Failed to read file {filepath}: {e}'}
                
                # Parse and apply hunks
                while i < len(lines) and not lines[i].strip().startswith('*** '):
                    hunk_line = lines[i]
                    
                    if hunk_line.startswith('@@'):
                        # Context line - find where to apply changes
                        context = hunk_line[2:].strip()
                        i += 1
                        
                        # Find the line in file that matches context
                        context_idx = None
                        for idx, fl in enumerate(file_lines):
                            if context in fl:
                                context_idx = idx
                                break
                        
                        if context_idx is None:
                            return {'success': False, 'error': f'Could not find context "{context}" in {filepath}'}
                        
                        # Apply changes relative to context
                        current_idx = context_idx + 1
                        while i < len(lines) and not lines[i].startswith('@@') and not lines[i].strip().startswith('*** '):
                            change_line = lines[i]
                            if change_line.startswith('-'):
                                # Remove line - find and remove it
                                to_remove = change_line[1:]
                                found = False
                                for search_idx in range(context_idx, min(context_idx + 20, len(file_lines))):
                                    if search_idx < len(file_lines) and to_remove in file_lines[search_idx]:
                                        file_lines.pop(search_idx)
                                        found = True
                                        break
                                if not found:
                                    return {'success': False, 'error': f'Could not find line to remove: "{to_remove}" in {filepath}'}
                            elif change_line.startswith('+'):
                                # Add line
                                to_add = change_line[1:]
                                file_lines.insert(current_idx, to_add)
                                current_idx += 1
                            i += 1
                    else:
                        i += 1
                
                # Write updated content
                try:
                    target_path = new_filepath if new_filepath else filepath
                    if os.path.dirname(target_path):
                        os.makedirs(os.path.dirname(target_path), exist_ok=True)
                    with open(target_path, 'w') as f:
                        f.write('\n'.join(file_lines))
                    
                    # If moved, delete original
                    if new_filepath and os.path.exists(filepath):
                        os.remove(filepath)
                        results.append(f'Updated and moved file: {filepath} -> {new_filepath}')
                    else:
                        results.append(f'Updated file: {filepath}')
                except Exception as e:
                    return {'success': False, 'error': f'Failed to write file {target_path}: {e}'}
            else:
                i += 1
        
        return {'success': True, 'message': '; '.join(results) if results else 'No changes applied'}
