import { zodResolver } from "@hookform/resolvers/zod";
import { useForm } from "react-hook-form";
import { toast } from "sonner";
import { z } from "zod";

import { FormDialog } from "@/components/shared/form-dialog";
import {
  FormControl,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";

import { useCreateSpaceProjectMutation } from "./workspace-mutations";

const createProjectSchema = z.object({
  name: z.string().trim().min(1, "Name is required."),
});

type CreateProjectValues = z.infer<typeof createProjectSchema>;

export function CreatePublicProjectDialog({
  targetSpaceId,
  listSpaceId,
  title,
  open,
  onOpenChange,
}: {
  targetSpaceId: string;
  listSpaceId: string;
  title: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const mutation = useCreateSpaceProjectMutation(listSpaceId);
  const form = useForm<CreateProjectValues>({
    resolver: zodResolver(createProjectSchema),
    defaultValues: { name: "" },
  });

  async function onSubmit(values: CreateProjectValues) {
    try {
      await mutation.mutateAsync({ name: values.name, spaceId: targetSpaceId });
      onOpenChange(false);
      toast.success(`Project "${values.name}" created.`);
      form.reset({ name: "" });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create project");
    }
  }

  return (
    <FormDialog
      description="Give the project a name to get started."
      form={form}
      isPending={mutation.isPending}
      onOpenChange={onOpenChange}
      onSubmit={onSubmit}
      open={open}
      submitLabel="Create"
      title={title}
    >
      <FormField
        control={form.control}
        name="name"
        render={({ field }) => (
          <FormItem>
            <FormLabel>Name</FormLabel>
            <FormControl>
              <Input placeholder="My Project" {...field} />
            </FormControl>
            <FormMessage />
          </FormItem>
        )}
      />
    </FormDialog>
  );
}
