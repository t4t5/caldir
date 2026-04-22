import type { APIRoute } from "astro";
import skill from "../data/skill.md?raw";

export const GET: APIRoute = async () => {
  return new Response(skill, {
    headers: { "Content-Type": "text/markdown; charset=utf-8" },
  });
};
