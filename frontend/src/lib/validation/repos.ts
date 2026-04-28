import { z } from "zod";

export const connectRepoFormSchema = z.object({
  github_repo_id: z
    .number({ message: "Repository ID is required" })
    .int()
    .positive("Repository ID must be a positive integer"),
  default_branch: z.string().optional(),
});

export type ConnectRepoFormValues = z.infer<typeof connectRepoFormSchema>;
