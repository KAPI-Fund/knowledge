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

import { useCreateTeamMutation } from "./workspace-mutations";

const createTeamSchema = z.object({
  name: z.string().trim().min(1, "Name is required."),
  slug: z.string().trim().min(1, "Slug is required."),
});

type CreateTeamValues = z.infer<typeof createTeamSchema>;

export function CreateTeamDialog({
  orgId,
  open,
  onOpenChange,
}: {
  orgId: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const mutation = useCreateTeamMutation(orgId);
  const form = useForm<CreateTeamValues>({
    resolver: zodResolver(createTeamSchema),
    defaultValues: { name: "", slug: "" },
  });

  async function onSubmit(values: CreateTeamValues) {
    try {
      await mutation.mutateAsync({ name: values.name, slug: values.slug });
      onOpenChange(false);
      toast.success(`Team "${values.name}" created.`);
      form.reset({ name: "", slug: "" });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create team");
    }
  }

  return (
    <FormDialog
      description="Create a team within this organization."
      form={form}
      isPending={mutation.isPending}
      onOpenChange={onOpenChange}
      onSubmit={onSubmit}
      open={open}
      submitLabel="Create"
      title="New team"
    >
      <FormField
        control={form.control}
        name="name"
        render={({ field }) => (
          <FormItem>
            <FormLabel>Name</FormLabel>
            <FormControl>
              <Input placeholder="Engineering" {...field} />
            </FormControl>
            <FormMessage />
          </FormItem>
        )}
      />
      <FormField
        control={form.control}
        name="slug"
        render={({ field }) => (
          <FormItem>
            <FormLabel>Slug</FormLabel>
            <FormControl>
              <Input placeholder="engineering" {...field} />
            </FormControl>
            <FormMessage />
          </FormItem>
        )}
      />
    </FormDialog>
  );
}
