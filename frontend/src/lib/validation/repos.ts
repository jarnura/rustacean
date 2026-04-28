import { z } from "zod";

export const connectRepoFormSchema = z.object({
  installation_id: z
    .number({ message: "Installation ID is required" })
    .int()
    .positive("Installation ID must be a positive integer"),
  github_repo_id: z
    .number({ message: "Repository ID is required" })
    .int()
    .positive("Repository ID must be a positive integer"),
  default_branch: z.string().optional(),
});

export type ConnectRepoFormValues = z.infer<typeof connectRepoFormSchema>;
