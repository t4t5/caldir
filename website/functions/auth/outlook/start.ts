interface Env {
  OUTLOOK_CLIENT_ID: string;
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

const SCOPES = "Calendars.ReadWrite User.Read offline_access";

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
    client_id: context.env.OUTLOOK_CLIENT_ID,
    redirect_uri: "https://caldir.org/auth/outlook/callback",
    response_type: "code",
    scope: SCOPES,
    state,
    response_mode: "query",
  });

  return Response.redirect(
    `https://login.microsoftonline.com/common/oauth2/v2.0/authorize?${params.toString()}`,
    302,
  );
};
