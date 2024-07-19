# Bin Reminder 

Script to automatically send notifications the night before a bin collection.

Environment variables need customising to an address, and the format of returned results may not match up depending on the local authority.

Set a line in cronjob to run similar to:

0 19 * * * /usr/local/bin/bin-reminder
