import type { APIRoute } from "astro";
import { getCollection } from "astro:content";
import { docsLinks } from "../data/docs-links";

export const GET: APIRoute = async () => {
  const docs = await getCollection("docs");
  const bySlug = new Map(docs.map((doc) => [doc.id, doc]));

  const body = docsLinks
    .map(({ slug }) => bySlug.get(slug)?.body?.trim())
    .filter((text): text is string => Boolean(text))
    .join("\n\n");

  return new Response(body + "\n", {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
};
