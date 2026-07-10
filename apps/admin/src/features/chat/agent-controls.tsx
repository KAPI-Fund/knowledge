import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";

import type { AgentMode, AvailableAgentSkill } from "./agent-types";

const AGENT_MODES: { value: AgentMode; label: string }[] = [
  { value: "fast", label: "Fast" },
  { value: "standard", label: "Standard" },
  { value: "deep", label: "Deep" },
];

export const SKILL_AUTO = "__auto__";
export const SKILL_NONE = "__none__";

export function AgentModeSelector({
  value,
  disabled = false,
  onChange,
}: {
  value: AgentMode;
  disabled?: boolean;
  onChange: (mode: AgentMode) => void;
}) {
  return (
    <Select disabled={disabled} onValueChange={(next) => onChange(next as AgentMode)} value={value}>
      <SelectTrigger className="h-8 w-28 text-xs" size="sm">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {AGENT_MODES.map((mode) => (
          <SelectItem key={mode.value} value={mode.value}>
            {mode.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function AgentSkillSelector({
  value,
  skills,
  disabled = false,
  onChange,
}: {
  value: string;
  skills: AvailableAgentSkill[];
  disabled?: boolean;
  onChange: (value: string) => void;
}) {
  return (
    <Select disabled={disabled} onValueChange={onChange} value={value}>
      <SelectTrigger className="h-8 w-44 text-xs" size="sm">
        <SelectValue placeholder="技能" />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value={SKILL_NONE}>无技能</SelectItem>
        <SelectItem value={SKILL_AUTO}>自动选择</SelectItem>
        {skills.map((skill) => (
          <SelectItem key={skill.id} value={skill.id}>
            {skill.name}
            <span className="ml-1 text-[10px] text-muted-foreground">
              {skill.source === "project" ? "项目" : "全局"}
            </span>
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function AgentWebToggle({
  value,
  disabled = false,
  onChange,
}: {
  value: boolean;
  disabled?: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center gap-1.5">
      <Switch
        checked={value}
        disabled={disabled}
        id="agent-web-toggle"
        onCheckedChange={onChange}
      />
      <Label className="cursor-pointer text-xs text-muted-foreground" htmlFor="agent-web-toggle">
        Web
      </Label>
    </div>
  );
}
