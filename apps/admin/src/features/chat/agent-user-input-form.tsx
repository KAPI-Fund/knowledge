import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Textarea } from "@/components/ui/textarea";

import type { AgentUserInputField, AgentUserInputRequest } from "./agent-types";

function initialValue(field: AgentUserInputField): unknown {
  if (field.defaultValue !== undefined && field.defaultValue !== null) {
    return field.defaultValue;
  }
  switch (field.type) {
    case "multi":
      return [];
    case "confirm":
      return null;
    default:
      return "";
  }
}

function initialValues(request: AgentUserInputRequest): Record<string, unknown> {
  const values: Record<string, unknown> = {};
  for (const field of request.fields) {
    values[field.id] = initialValue(field);
  }
  return values;
}

export function AgentUserInputForm({
  request,
  disabled = false,
  onSubmit,
}: {
  request: AgentUserInputRequest;
  disabled?: boolean;
  onSubmit: (values: Record<string, unknown>) => void;
}) {
  const [values, setValues] = useState<Record<string, unknown>>(() => initialValues(request));

  const setValue = (fieldId: string, value: unknown) => {
    setValues((current) => ({ ...current, [fieldId]: value }));
  };

  return (
    <form
      className="space-y-4 rounded-md border border-border/60 bg-background/60 p-3 text-sm"
      onSubmit={(event) => {
        event.preventDefault();
        if (!disabled) {
          onSubmit(values);
        }
      }}
    >
      <div className="space-y-1">
        <p className="font-medium">{request.title}</p>
        {request.description ? (
          <p className="text-xs text-muted-foreground">{request.description}</p>
        ) : null}
      </div>

      {request.fields.map((field) => (
        <div className="space-y-1.5" key={field.id}>
          <Label className="text-xs" htmlFor={`agent-field-${field.id}`}>
            {field.label}
          </Label>
          {field.description ? (
            <p className="text-[11px] text-muted-foreground">{field.description}</p>
          ) : null}
          <AgentUserInputFieldControl
            disabled={disabled}
            field={field}
            onChange={(value) => setValue(field.id, value)}
            value={values[field.id]}
          />
        </div>
      ))}

      <Button disabled={disabled} size="sm" type="submit">
        提交
      </Button>
    </form>
  );
}

function AgentUserInputFieldControl({
  field,
  value,
  disabled,
  onChange,
}: {
  field: AgentUserInputField;
  value: unknown;
  disabled: boolean;
  onChange: (value: unknown) => void;
}) {
  if (field.type === "text") {
    return (
      <Input
        disabled={disabled}
        id={`agent-field-${field.id}`}
        onChange={(event) => onChange(event.target.value)}
        placeholder={field.placeholder ?? undefined}
        value={typeof value === "string" ? value : ""}
      />
    );
  }

  if (field.type === "textarea") {
    return (
      <Textarea
        disabled={disabled}
        id={`agent-field-${field.id}`}
        onChange={(event) => onChange(event.target.value)}
        placeholder={field.placeholder ?? undefined}
        rows={4}
        value={typeof value === "string" ? value : ""}
      />
    );
  }

  if (field.type === "single") {
    return (
      <RadioGroup
        disabled={disabled}
        onValueChange={(next) => onChange(next)}
        value={typeof value === "string" ? value : ""}
      >
        {(field.options ?? []).map((option) => (
          <div className="flex items-start gap-2" key={option.value}>
            <RadioGroupItem
              className="mt-0.5"
              id={`agent-field-${field.id}-${option.value}`}
              value={option.value}
            />
            <Label
              className="flex-1 cursor-pointer font-normal"
              htmlFor={`agent-field-${field.id}-${option.value}`}
            >
              {option.label}
              {option.recommended ? (
                <span className="ml-1 text-[10px] text-muted-foreground">（推荐）</span>
              ) : null}
              {option.description ? (
                <span className="block text-[11px] text-muted-foreground">
                  {option.description}
                </span>
              ) : null}
            </Label>
          </div>
        ))}
      </RadioGroup>
    );
  }

  if (field.type === "multi") {
    const selected = Array.isArray(value) ? (value as unknown[]).map(String) : [];
    return (
      <div className="space-y-1.5">
        {(field.options ?? []).map((option) => {
          const checked = selected.includes(option.value);
          return (
            <div className="flex items-start gap-2" key={option.value}>
              <Checkbox
                checked={checked}
                className="mt-0.5"
                disabled={disabled}
                id={`agent-field-${field.id}-${option.value}`}
                onCheckedChange={(next) => {
                  const nextSelected = next
                    ? [...selected, option.value]
                    : selected.filter((entry) => entry !== option.value);
                  onChange(nextSelected);
                }}
              />
              <Label
                className="flex-1 cursor-pointer font-normal"
                htmlFor={`agent-field-${field.id}-${option.value}`}
              >
                {option.label}
                {option.description ? (
                  <span className="block text-[11px] text-muted-foreground">
                    {option.description}
                  </span>
                ) : null}
              </Label>
            </div>
          );
        })}
      </div>
    );
  }

  return (
    <RadioGroup
      disabled={disabled}
      onValueChange={(next) => onChange(next === "true")}
      value={value === true ? "true" : value === false ? "false" : ""}
    >
      <div className="flex items-center gap-2">
        <RadioGroupItem id={`agent-field-${field.id}-yes`} value="true" />
        <Label className="cursor-pointer font-normal" htmlFor={`agent-field-${field.id}-yes`}>
          确认
        </Label>
      </div>
      <div className="flex items-center gap-2">
        <RadioGroupItem id={`agent-field-${field.id}-no`} value="false" />
        <Label className="cursor-pointer font-normal" htmlFor={`agent-field-${field.id}-no`}>
          拒绝
        </Label>
      </div>
    </RadioGroup>
  );
}
