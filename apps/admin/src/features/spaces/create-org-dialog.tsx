import { zodResolver } from "@hookform/resolvers/zod";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useForm } from "react-hook-form";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { z } from "zod";

import { FormDialog } from "@/components/shared/form-dialog";
import {
  FormControl,
  FormDescription,
  FormField,
  FormItem,
  FormLabel,
  FormMessage,
} from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import { createOrg } from "@/features/shared/tenancy-api";

function slugify(value: string) {
  return value
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

const createOrgSchema = z.object({
  name: z.string().trim().min(1, "Name is required."),
  slug: z.string().trim(),
});

type CreateOrgValues = z.infer<typeof createOrgSchema>;

export function CreateOrgDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const form = useForm<CreateOrgValues>({
    resolver: zodResolver(createOrgSchema),
    defaultValues: { name: "", slug: "" },
  });
  const watchedName = form.watch("name");

  const mutation = useMutation({
    mutationFn: (values: CreateOrgValues) =>
      createOrg({ name: values.name, slug: values.slug || slugify(values.name) }),
    onSuccess: async (org) => {
      await queryClient.invalidateQueries({ queryKey: ["spaces"] });
      onOpenChange(false);
      toast.success(`Organization "${org.name}" created.`);
      form.reset({ name: "", slug: "" });
      navigate(`/orgs/${org.id}`);
    },
  });

  async function onSubmit(values: CreateOrgValues) {
    try {
      await mutation.mutateAsync(values);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Failed to create organization");
    }
  }

  return (
    <FormDialog
      description="Create a shared workspace for teams and public knowledge bases."
      form={form}
      isPending={mutation.isPending}
      onOpenChange={onOpenChange}
      onSubmit={onSubmit}
      open={open}
      submitLabel="Create organization"
      title="New organization"
    >
      <FormField
        control={form.control}
        name="name"
        render={({ field }) => (
          <FormItem>
            <FormLabel>Name</FormLabel>
            <FormControl>
              <Input placeholder="Acme Research" {...field} />
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
              <Input placeholder={slugify(watchedName) || "acme-research"} {...field} />
            </FormControl>
            <FormDescription>Leave blank to derive from the name.</FormDescription>
            <FormMessage />
          </FormItem>
        )}
      />
    </FormDialog>
  );
}
