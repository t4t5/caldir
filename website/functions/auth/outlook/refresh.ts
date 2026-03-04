interface Env {
  OUTLOOK_CLIENT_ID: string;
  OUTLOOK_CLIENT_SECRET: string;
}

export const onRequestPost: PagesFunction<Env> = async (context) => {
  const body = (await context.request.json()) as { refresh_token?: string };

  if (!body.refresh_token) {
    return Response.json(
      { error: "Missing refresh_token" },
      { status: 400 },
    );
  }

  const tokenResponse = await fetch(
    "https://login.microsoftonline.com/common/oauth2/v2.0/token",
    {
      method: "POST",
      headers: { "Content-Type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams({
        refresh_token: body.refresh_token,
        client_id: context.env.OUTLOOK_CLIENT_ID,
        client_secret: context.env.OUTLOOK_CLIENT_SECRET,
        grant_type: "refresh_token",
      }),
    },
  );

  if (!tokenResponse.ok) {
    const errorText = await tokenResponse.text();
    console.error("Token refresh failed:", errorText);
    return Response.json(
      { error: "Failed to refresh token" },
      { status: 502 },
    );
  }

  // Microsoft returns a new refresh_token on each refresh (unlike Google)
  const tokens = (await tokenResponse.json()) as {
    access_token: string;
    refresh_token: string;
    expires_in: number;
  };

  return Response.json({
    access_token: tokens.access_token,
    refresh_token: tokens.refresh_token,
    expires_in: tokens.expires_in,
  });
};
