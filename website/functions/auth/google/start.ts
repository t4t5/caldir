interface Env {
  GOOGLE_CLIENT_ID: string;
  HMAC_SECRET: string;
}

async function hmacSign(data: string, secret: string): Promise<string> {
  const encoder = new TextEncoder();
  const key = await crypto.subtle.importKey(
    "raw",
    encoder.encode(secret),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign("HMAC", key, encoder.encode(data));
  return btoa(String.fromCharCode(...new Uint8Array(signature)))
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");
}

export const onRequestGet: PagesFunction<Env> = async (context) => {
  const url = new URL(context.request.url);
  const port = url.searchParams.get("port");

  if (!port || !/^\d+$/.test(port)) {
    return new Response("Missing or invalid port parameter", { status: 400 });
  }

  const portNum = parseInt(port, 10);
  if (portNum < 1024 || portNum > 65535) {
    return new Response("Port must be between 1024 and 65535", {
      status: 400,
    });
  }

  const timestamp = Math.floor(Date.now() / 1000).toString();
  const nonce = crypto.randomUUID();
  const payload = `${port}:${timestamp}:${nonce}`;
  const signature = await hmacSign(payload, context.env.HMAC_SECRET);
  const state = `${payload}:${signature}`;

  const params = new URLSearchParams({
    client_id: context.env.GOOGLE_CLIENT_ID,
    redirect_uri: "https://caldir.org/auth/google/callback",
    response_type: "code",
    scope: "https://www.googleapis.com/auth/calendar",
    access_type: "offline",
    prompt: "consent",
    state,
  });

  return Response.redirect(
    `https://accounts.google.com/o/oauth2/v2/auth?${params.toString()}`,
    302,
  );
};
