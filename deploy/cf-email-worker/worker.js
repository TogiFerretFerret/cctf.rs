export default {
	async email(message, env, ctx) {
		const raw = await new Response(message.raw).text();
		const res = await fetch(env.CCTF_WEBHOOK_URL, {
			method: "POST",
			headers: {
				"content-type": "message/rfc822",
				"authorization" : `Bearer ${env.CCTF_WEBHOOK_SECRET}`,
				"x-mail-from": message.from,
				"x-mail-to": message.to,
			},
			body: raw,
		});
		if (!res.ok) message.setReject("temporary failure");
	},
};
