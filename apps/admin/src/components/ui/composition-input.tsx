import * as React from "react";

import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";

/**
 * Controlled text fields that survive IME (输入法) composition.
 *
 * The canvas rebuilds every node from the document model on each keystroke, so a
 * raw controlled `value` gets written back to the DOM *mid-composition*. The
 * browser then commits its composition buffer on top of that stale value, which
 * duplicates characters for CJK/IME users. These wrappers buffer the value
 * locally, never propagate or accept an external value while composing, and emit
 * a single committed value on `compositionend`.
 */

type FieldElement = HTMLInputElement | HTMLTextAreaElement;

interface CompositionState<T extends FieldElement> {
  buffer: string;
  handlers: {
    value: string;
    onChange: (event: React.ChangeEvent<T>) => void;
    onCompositionStart: () => void;
    onCompositionEnd: (event: React.CompositionEvent<T>) => void;
  };
}

function useCompositionField<T extends FieldElement>(
  value: string,
  onValueChange: (value: string) => void,
): CompositionState<T> {
  const [buffer, setBuffer] = React.useState(value);
  const composingRef = React.useRef(false);

  // Adopt external updates only when we're not composing, so an async document
  // round-trip can't clobber the in-progress IME buffer.
  React.useEffect(() => {
    if (composingRef.current) return;
    setBuffer(value);
  }, [value]);

  const onChange = React.useCallback(
    (event: React.ChangeEvent<T>) => {
      const next = event.target.value;
      setBuffer(next);
      if (!composingRef.current) {
        onValueChange(next);
      }
    },
    [onValueChange],
  );

  const onCompositionStart = React.useCallback(() => {
    composingRef.current = true;
  }, []);

  const onCompositionEnd = React.useCallback(
    (event: React.CompositionEvent<T>) => {
      composingRef.current = false;
      const next = (event.target as T).value;
      setBuffer(next);
      onValueChange(next);
    },
    [onValueChange],
  );

  return {
    buffer,
    handlers: { value: buffer, onChange, onCompositionStart, onCompositionEnd },
  };
}

type InputProps = Omit<React.ComponentProps<typeof Input>, "value" | "onChange"> & {
  value: string;
  onValueChange: (value: string) => void;
};

export function CompositionInput({ value, onValueChange, ...props }: InputProps) {
  const { handlers } = useCompositionField<HTMLInputElement>(value, onValueChange);
  return <Input {...props} {...handlers} />;
}

type TextareaProps = Omit<
  React.ComponentProps<typeof Textarea>,
  "value" | "onChange"
> & {
  value: string;
  onValueChange: (value: string) => void;
};

export function CompositionTextarea({ value, onValueChange, ...props }: TextareaProps) {
  const { handlers } = useCompositionField<HTMLTextAreaElement>(value, onValueChange);
  return <Textarea {...props} {...handlers} />;
}
