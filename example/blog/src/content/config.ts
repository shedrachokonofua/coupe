import { defineCollection, z } from "astro:content";

const posts = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string(),
    excerpt: z.string(),
    date: z.string(),
    author: z.string(),
    tags: z.array(z.string()),
    readTime: z.string(),
  }),
});

export const collections = {
  posts,
};
