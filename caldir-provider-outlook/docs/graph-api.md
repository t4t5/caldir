---
title: "Working with calendars and events using the Microsoft Graph API - Microsoft Graph v1.0"
source: "https://learn.microsoft.com/en-us/graph/api/resources/calendar-overview?view=graph-rest-1.0"
description: "Learn how to manage calendars and events with the Calendar API in Microsoft Graph."
---

## Working with calendars and events using the Microsoft Graph API

The Microsoft Graph Calendar API provides [calendar](https://learn.microsoft.com/en-us/graph/api/resources/calendar?view=graph-rest-1.0), [calendarGroup](https://learn.microsoft.com/en-us/graph/api/resources/calendargroup?view=graph-rest-1.0), [event](https://learn.microsoft.com/en-us/graph/api/resources/event?view=graph-rest-1.0), and other resources that enable you to create events and meetings, find workable meeting times, manage attendees, and more. With the Calendar API, you can build a variety of experiences with calendar data.

## Manage events and meetings

The [event](https://learn.microsoft.com/en-us/graph/api/resources/event?view=graph-rest-1.0) type represents a scheduled occurrence on a calendar, such as a meeting, holiday, or time block. Meetings, such as team meetings or one-on-ones, are all represented by **event** resources. You can directly manage the event lifecycle by creating, canceling, and deleting events directly, among other actions. Also, you can create draft event messages, send them, forward them, and create draft replies, and more. By working with event messages, you enable the user to take an active role in creating events and meetings. You also enable them to communicate to meeting originators, other recipients, and attendees.

### Working directly with events

The Microsoft Graph API provides methods for operations such as creating, updating, deleting, and canceling events. The following table lists some common lifecycle event use cases and the APIs that Microsoft Graph provides for working with them.

| Use case | Verb | Example URL |
| --- | --- | --- |
| [Create an event.](https://learn.microsoft.com/en-us/graph/api/user-post-events?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/events |
| [Delete an event from a calendar.](https://learn.microsoft.com/en-us/graph/api/event-delete?view=graph-rest-1.0) | DELETE | /users/{id \| userPrincipalName}/events/{id} |
| [Cancel an event and send a cancellation message.](https://learn.microsoft.com/en-us/graph/api/event-cancel?view=graph-rest-1.0)   **Note**: Specify the optional cancellation message in the request body. | POST | /users/{id \| userPrincipalName}/events/{id}/cancel |
| [Update an event.](https://learn.microsoft.com/en-us/graph/api/event-update?view=graph-rest-1.0)   **Note**: Specify the event details to update in the [request body](https://learn.microsoft.com/en-us/graph/api/event-update?view=graph-rest-1.0#request-body). | PATCH | /users/{id \| userPrincipalName}/events/{id} |
| [Accept an event.](https://learn.microsoft.com/en-us/graph/api/event-accept?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/events/{id}/accept |
| [Tentatively accept an event.](https://learn.microsoft.com/en-us/graph/api/event-tentativelyaccept?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/events/{id}/tentativelyAccept |
| [Decline an event.](https://learn.microsoft.com/en-us/graph/api/event-decline?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/events/{id}/decline |
| [Dismiss an event reminder.](https://learn.microsoft.com/en-us/graph/api/event-dismissreminder?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/events/{id}/dismissReminder |
| [Snooze an event reminder.](https://learn.microsoft.com/en-us/graph/api/event-snoozereminder?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/events/{id}/snoozeReminder |

### Working with event messages

The [eventMessage](https://learn.microsoft.com/en-us/graph/api/resources/eventmessage?view=graph-rest-1.0) resource is an abstract type that represents meeting requests, cancellations, and responses. Responses are generated when the message recipient accepts, tentatively accepts, or declines the request. Handling [eventMessageRequest](https://learn.microsoft.com/en-us/graph/api/resources/eventmessagerequest?view=graph-rest-1.0) and [eventMessageResponse](https://learn.microsoft.com/en-us/graph/api/resources/eventmessageresponse?view=graph-rest-1.0) moves the event through its lifecycle. The messaging APIs in the Calendar API support both MIME and JSON content.

The following table lists some common event message use cases and the APIs for working with them.

| Use case | Verb | Example URL |
| --- | --- | --- |
| [Send an existing draft message.](https://learn.microsoft.com/en-us/graph/api/message-send?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/send |
| [Create a draft reply.](https://learn.microsoft.com/en-us/graph/api/message-createreply?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/createReply |
| [Reply to an event message.](https://learn.microsoft.com/en-us/graph/api/message-reply?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/reply |
| [Create a draft reply-all message.](https://learn.microsoft.com/en-us/graph/api/message-createreplyall?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/createReplyAll |
| [Reply to all in an event message.](https://learn.microsoft.com/en-us/graph/api/message-replyall?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/replyAll |
| [Create a draft forward.](https://learn.microsoft.com/en-us/graph/api/message-createforward?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/createForward |
| [Forward an event message.](https://learn.microsoft.com/en-us/graph/api/message-forward?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/messages/{id}/forward |

## Adding and removing attachments

The abstract [attachment](https://learn.microsoft.com/en-us/graph/api/resources/attachment?view=graph-rest-1.0) type serves as a base for files, items, and references that are attached to events, messages, and posts. You can view the attachments for an event, for example, with the [List attachments](https://learn.microsoft.com/en-us/graph/api/event-list-attachments?view=graph-rest-1.0) method. You can delete an attachment with the [Delete attachment](https://learn.microsoft.com/en-us/graph/api/attachment-delete?view=graph-rest-1.0) method. Events in group calendars don't support attachments.

### Attachment types

The [fileAttachment](https://learn.microsoft.com/en-us/graph/api/resources/fileattachment?view=graph-rest-1.0), [itemAttachment](https://learn.microsoft.com/en-us/graph/api/resources/itemattachment?view=graph-rest-1.0), and [referenceAttatchment](https://learn.microsoft.com/en-us/graph/api/resources/referenceattachment?view=graph-rest-1.0) types represent the three kinds of items that can be attached to calendar items. An **itemAttachment** object represents a contact, event, or message that is directly attached to a user event, message, or post. A **fileAttachment** represents a file that is directly attached. A **referenceAttachment** represents an item, such as a Word document or text file, that is located on a OneDrive for work or school cloud drive or other supported storage location. To see all of the attachments for an [event](https://learn.microsoft.com/en-us/graph/api/resources/event?view=graph-rest-1.0), for example, you can use the [GET /users/{id | userPrincipalName}/events/{id}/attachments](https://learn.microsoft.com/en-us/graph/api/event-list-attachments?view=graph-rest-1.0) endpoint.

### Uploading attachments

You can directly upload attachments less than 3 MB in size to an event for a user with the [Add attachment](https://learn.microsoft.com/en-us/graph/api/event-post-attachments?view=graph-rest-1.0) method. For an attachment that is larger than 3 MB, however, you must use the [attachment: createUploadSession](https://learn.microsoft.com/en-us/graph/api/attachment-createuploadsession?view=graph-rest-1.0) method to get an upload URL that you use to iteratively upload the attachment.

## Work with calendars, calendar groups, and Outlook categories

With the Calendar API, you can create, read, update, and delete calendars, create and view calendar events, get free/busy information for users, and find suggested meeting times.

The Calendar API provides methods to operate on calendars and calendar groups. The following table shows some use cases with selected URLs.

> **Note**: Many of the methods shown in the following table have other URLs for related use cases. For example, to update a user's calendar in a specific calendar group, send a PATCH operation with the URL `/users/{id | userPrincipalName}/calendarGroups/{id}/calendars/{id}`.

| Use case | Verb | Example URL |
| --- | --- | --- |
| [List calendars for a user.](https://learn.microsoft.com/en-us/graph/api/user-list-calendars?view=graph-rest-1.0) | GET | /users/{id \| userPrincipalName}/calendars |
| [List a user's calendars in a group.](https://learn.microsoft.com/en-us/graph/api/user-list-calendars?view=graph-rest-1.0) | GET | /users/{id \| userPrincipalName}/calendarGroups/{calendarGroupId}/calendars |
| [Create a calendar.](https://learn.microsoft.com/en-us/graph/api/user-post-calendars?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/calendars |
| [Get a calendar.](https://learn.microsoft.com/en-us/graph/api/calendar-get?view=graph-rest-1.0) | GET | /users/{id \| userPrincipalName}/calendars/{id} |
| [Update a calendar.](https://learn.microsoft.com/en-us/graph/api/calendar-update?view=graph-rest-1.0) | PATCH | /users/{id \| userPrincipalName}/calendars/{id} |
| [Delete a calendar.](https://learn.microsoft.com/en-us/graph/api/calendar-delete?view=graph-rest-1.0) | DELETE | /users/{id \| userPrincipalName}/calendars/{id} |
| [Create a calendar group.](https://learn.microsoft.com/en-us/graph/api/user-post-calendargroups?view=graph-rest-1.0) | POST | /users/{id \| userPrincipalName}/calendarGroups |
| [Get a calendar group.](https://learn.microsoft.com/en-us/graph/api/calendargroup-get?view=graph-rest-1.0) | GET | /users/{id \| userPrincipalName}/calendarGroups/{id} |
| [Update a calendar group.](https://learn.microsoft.com/en-us/graph/api/calendargroup-update?view=graph-rest-1.0) | PATCH | /users/{id \| userPrincipalName}/calendarGroups/{id} |
| [Delete a calendar group.](https://learn.microsoft.com/en-us/graph/api/calendargroup-delete?view=graph-rest-1.0) | DELETE | /users/{id \| userPrincipalName}/calendarGroups/{id} |

### Free/busy data and meeting times

Two of the core functions of calendaring are to find free/busy information and find meeting times in order to schedule meetings. The Calendar API provides the [Get free/busy schedule](https://learn.microsoft.com/en-us/graph/api/calendar-getschedule?view=graph-rest-1.0) method that returns a collection of [scheduleInformation](https://learn.microsoft.com/en-us/graph/api/resources/scheduleinformation?view=graph-rest-1.0) objects for a time period and a collection of users, resources, or distribution lists. You can present this information to the user so that they can manually pick an appropriate time at which to schedule a meeting. Use the [user: findMeetingTimes](https://learn.microsoft.com/en-us/graph/api/user-findmeetingtimes?view=graph-rest-1.0) method to get a [meetingTimeSuggestionResult](https://learn.microsoft.com/en-us/graph/api/resources/meetingtimesuggestionsresult?view=graph-rest-1.0) that contains a collection of [meetingTimeSuggestion](https://learn.microsoft.com/en-us/graph/api/resources/meetingtimesuggestion?view=graph-rest-1.0) objects that represent detailed information about proposed meeting times for the participants and constraints that you sent.

### Outlook categories

A calendar category is a combination of a description and a **categoryColor** that together define a category for an Outlook item and control how Outlook displays the item. Outlook users can group messages and events, for example, by category. For more information, see [outlookCategory](https://learn.microsoft.com/en-us/graph/api/resources/outlookcategory?view=graph-rest-1.0).

### Calendar permissions

When users share calendars with other users from within Outlook clients, they can control the calendar items that the recipients can view or edit. The [calendarPermissions](https://learn.microsoft.com/en-us/graph/api/resources/calendar?view=graph-rest-1.0#relationships) relationship contains permissions for every user with whom a user shared their calendar. This relationship allows you to, for example, see which users can view free/busy information for the owner, view all calendar information, or edit events on the calendar.

## Work with open extensions and extended properties

[Open extensions](https://learn.microsoft.com/en-us/graph/api/resources/opentypeextension?view=graph-rest-1.0), formerly known as Office 365 data extensions, represent the preferred way to store and access custom data for resources in a user's mailbox. If an Outlook MAPI property isn't available in the Microsoft Graph API metadata, then you can fall back to Outlook extended properties. For more information, see [Outlook extended properties overview](https://learn.microsoft.com/en-us/graph/api/resources/extended-properties-overview?view=graph-rest-1.0).

## Next steps

The Calendar API in Microsoft Graph allows you to build a range of experiences with calendar data. To learn more:

- Drill down on the methods and properties of the resources most helpful to your scenario.
- Try the API in the [Graph Explorer](https://developer.microsoft.com/graph/graph-explorer).
