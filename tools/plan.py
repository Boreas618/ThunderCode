from typing import Optional
from enum import Enum

from tools.base import Tool


class StepStatus(Enum):
    """Status of a plan step."""
    PENDING = "pending"
    IN_PROGRESS = "in_progress"
    COMPLETED = "completed"


class UpdatePlanTool(Tool):
    """
    Tool for updating the task plan.
    
    Provides an optional explanation and a list of plan items,
    each with a step and status. At most one step can be in_progress at a time.
    """
    
    # Store the current plan state
    _current_plan: list[dict] = []
    _explanation: Optional[str] = None
    
    @property
    def name(self) -> str:
        return "update_plan"
    
    @property
    def description(self) -> str:
        return (
            "Updates the task plan. "
            "Provide an optional explanation and a list of plan items, each with a step and status. "
            "At most one step can be in_progress at a time."
        )
    
    @property
    def parameters(self) -> dict:
        return {
            "type": "object",
            "properties": {
                "explanation": {
                    "type": "string",
                    "description": "Optional explanation for plan changes."
                },
                "plan": {
                    "type": "array",
                    "description": "The list of steps",
                    "items": {
                        "type": "object",
                        "properties": {
                            "step": {
                                "type": "string",
                                "description": "Description of the step."
                            },
                            "status": {
                                "type": "string",
                                "description": "One of: pending, in_progress, completed"
                            }
                        },
                        "required": ["step", "status"],
                        "additionalProperties": False
                    }
                }
            },
            "required": ["plan"],
            "additionalProperties": False
        }
    
    def execute(
        self,
        plan: list[dict],
        explanation: Optional[str] = None,
    ) -> dict:
        if not plan:
            return {"success": False, "error": "plan is required and cannot be empty"}
        
        # Validate each plan item
        valid_statuses = {"pending", "in_progress", "completed"}
        in_progress_count = 0
        
        validated_plan = []
        for i, item in enumerate(plan):
            if not isinstance(item, dict):
                return {"success": False, "error": f"Plan item {i + 1} must be an object"}
            
            step = item.get("step")
            status = item.get("status")
            
            if not step:
                return {"success": False, "error": f"Plan item {i + 1} missing 'step' field"}
            
            if not status:
                return {"success": False, "error": f"Plan item {i + 1} missing 'status' field"}
            
            if status not in valid_statuses:
                return {
                    "success": False,
                    "error": f"Plan item {i + 1} has invalid status '{status}'. Must be one of: {', '.join(valid_statuses)}"
                }
            
            if status == "in_progress":
                in_progress_count += 1
            
            validated_plan.append({
                "step": step,
                "status": status
            })
        
        # Check constraint: at most one step can be in_progress
        if in_progress_count > 1:
            return {
                "success": False,
                "error": f"At most one step can be in_progress at a time, but found {in_progress_count}"
            }
        
        # Update the stored plan
        UpdatePlanTool._current_plan = validated_plan
        UpdatePlanTool._explanation = explanation
        
        # Format output
        output_lines = []
        if explanation:
            output_lines.append(f"Explanation: {explanation}")
            output_lines.append("")
        
        output_lines.append("Plan:")
        for i, item in enumerate(validated_plan, start=1):
            status_icon = {
                "pending": "○",
                "in_progress": "◐",
                "completed": "●"
            }.get(item["status"], "?")
            output_lines.append(f"  {i}. [{status_icon}] {item['step']} ({item['status']})")
        
        # Calculate progress
        total = len(validated_plan)
        completed = sum(1 for item in validated_plan if item["status"] == "completed")
        in_progress = sum(1 for item in validated_plan if item["status"] == "in_progress")
        pending = sum(1 for item in validated_plan if item["status"] == "pending")
        
        return {
            "success": True,
            "message": "\n".join(output_lines),
            "summary": {
                "total": total,
                "completed": completed,
                "in_progress": in_progress,
                "pending": pending,
                "progress_percent": round(completed / total * 100, 1) if total > 0 else 0
            }
        }
    
    @classmethod
    def get_current_plan(cls) -> list[dict]:
        """Get the current plan state."""
        return cls._current_plan.copy()
    
    @classmethod
    def get_explanation(cls) -> Optional[str]:
        """Get the current explanation."""
        return cls._explanation
    
    @classmethod
    def clear_plan(cls) -> None:
        """Clear the current plan."""
        cls._current_plan = []
        cls._explanation = None
