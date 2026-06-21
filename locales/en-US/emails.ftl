# Verification & Welcome
email-welcome-subject = Welcome to {$ctf_name}!
email-welcome-body =
    <div style="font-family: sans-serif; padding: 20px; max-width: 600px; margin: auto; border: 1px solid #eee; border-radius: 10px;">
        <h2 style="color: #0070f3; margin-top: 0;">Welcome to {$ctf_name}!</h2>
        <p style="color: #333; line-height: 1.5;">Thank you for registering. We are excited to have you compete in {$ctf_name}.</p>
        <p style="color: #333; line-height: 1.5;">Click the button below to verify your account and join the game:</p>
        <div style="text-align: center; margin: 30px 0;">
            <a href="{$url}" style="background-color: #0070f3; color: white; padding: 12px 24px; text-decoration: none; border-radius: 5px; font-weight: bold; display: inline-block;">Verify My Account</a>
        </div>
        <hr style="border: 0; border-top: 1px solid #eee; margin: 20px 0;" />
        <p style="font-size: 12px; color: #666; text-align: center;">If you did not sign up for {$ctf_name}, you can safely ignore this email.</p>
    </div>

# Password Reset
email-reset-subject = Reset your {$ctf_name} Password
email-reset-body =
    <div style="font-family: sans-serif; padding: 20px; max-width: 600px; margin: auto; border: 1px solid #eee; border-radius: 10px;">
        <h2 style="color: #ff4d4f; margin-top: 0;">Password Reset Request</h2>
        <p style="color: #333; line-height: 1.5;">We received a request to reset your password for your {$ctf_name} account.</p>
        <p style="color: #333; line-height: 1.5;">Click the link below to set a new password. This link is valid for <strong>1 hour</strong>.</p>
        <div style="text-align: center; margin: 30px 0;">
            <a href="{$url}" style="background-color: #ff4d4f; color: white; padding: 12px 24px; text-decoration: none; border-radius: 5px; font-weight: bold; display: inline-block;">Reset Password</a>
        </div>
        <hr style="border: 0; border-top: 1px solid #eee; margin: 20px 0;" />
        <p style="font-size: 12px; color: #666; text-align: center;">If you did not request this, please ignore this email. Your password will remain secure.</p>
    </div>

# Invite to Team
email-team-invite-subject = You have been invited to join Team {$team_name}
email-team-invite-body =
    <div style="font-family: sans-serif; padding: 20px; max-width: 600px; margin: auto; border: 1px solid #eee; border-radius: 10px;">
        <h2 style="color: #52c41a; margin-top: 0;">Team Invitation</h2>
        <p style="color: #333; line-height: 1.5;">You have been invited to compete with <strong>{$team_name}</strong> in the {$ctf_name} competition!</p>
        <p style="color: #333; line-height: 1.5;">Log in to your account and enter the following invite code to join their team:</p>
        <div style="text-align: center; margin: 30px 0;">
            <code style="background-color: #f5f5f5; border: 1px dashed #d9d9d9; padding: 10px 20px; font-size: 18px; font-weight: bold; font-family: monospace; border-radius: 5px; display: inline-block;">{$invite_code}</code>
        </div>
        <hr style="border: 0; border-top: 1px solid #eee; margin: 20px 0;" />
        <p style="font-size: 12px; color: #666; text-align: center;">See you on the scoreboard!</p>
    </div>
