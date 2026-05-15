# Curiohoster liveValue Socket.IO examples

Date: 2026-05-15

## Source

RSS live value tag under investigation:

```xml
<podcast:liveValue
    uri="https://curiohoster.com/event?event_id=1873a383-d918-44e1-b6ff-c3598189dab6"
    protocol="socket.io"/>
```

The compatible Socket.IO client behavior is:

1. Start the Engine.IO polling session with `event_id` already present in the query string.
2. Connect to the Socket.IO namespace `/event`.
3. Listen for the `remoteValue` event.

The `event_id` query parameter must be present on the initial Engine.IO request. Adding it only after the `sid` is assigned can produce a misleading empty `{}` `remoteValue` payload.

Example raw polling sequence:

```sh
curl -i 'https://curiohoster.com/socket.io/?event_id=1873a383-d918-44e1-b6ff-c3598189dab6&EIO=4&transport=polling'
curl -i -X POST 'https://curiohoster.com/socket.io/?event_id=1873a383-d918-44e1-b6ff-c3598189dab6&EIO=4&transport=polling&sid=<engine-io-sid>' --data-binary '40/event,'
curl -i 'https://curiohoster.com/socket.io/?event_id=1873a383-d918-44e1-b6ff-c3598189dab6&EIO=4&transport=polling&sid=<engine-io-sid>'
```

The response packet shape is:

```text
42/event,["remoteValue", { ...payload... }]
```

## Example 1: chapter/value prompt

Observed at 2026-05-15T00:26:40Z.

Raw Socket.IO packet as observed:

```text
42/event,["remoteValue",{"image":"https://feed.homegrownhits.xyz/assets/images/gif/hgh-lava.gif","title":"Value for Value","line":["Give it to 'em, baby!","Boosting is loving, so boost a couple sats to the musicians you love and the DJs you adore using a value-enabled Podcasting 2.0 app."],"link":{"text":"NudePodcastApps.com","url":"https://podcastindex.org/apps"},"description":"","value":{"type":"lightning","method":"keysend","destinations":[{"type":"node","name":"DuhLaurien","address":"0368fed1c2fc35393228091c3ed1558090bfefc2ba5f57d8e025784f7fbc705dd5","split":"49"},{"name":"BoostAfterBoost","type":"node","address":"03ecb3ee55ba6324d40bea174de096dc9134cb35d990235723b37ae9b5c49f4f53","split":"1"},{"name":"MaryKateUltra","address":"0325ab94e785e40877fe7421ec9a523bbb6021663dfb5c18987f40e17d5d507921","type":"node","customKey":"","customValue":"","split":"50"}]},"type":"chapter","eventGuid":"1873a383-d918-44e1-b6ff-c3598189dab6","eventAPI":"https://curiohoster.com/api/sk/event/lookup","blockGuid":"bcef5cc4-f534-4832-aa89-d9a26f5a0ae6","startTime":1133.701,"eventTimestamp":1778198673942,"duration":0}]
```

```json
{
  "image": "https://feed.homegrownhits.xyz/assets/images/gif/hgh-lava.gif",
  "title": "Value for Value",
  "line": [
    "Give it to 'em, baby!",
    "Boosting is loving, so boost a couple sats to the musicians you love and the DJs you adore using a value-enabled Podcasting 2.0 app."
  ],
  "link": {
    "text": "NudePodcastApps.com",
    "url": "https://podcastindex.org/apps"
  },
  "description": "",
  "value": {
    "type": "lightning",
    "method": "keysend",
    "destinations": [
      {
        "type": "node",
        "name": "DuhLaurien",
        "address": "0368fed1c2fc35393228091c3ed1558090bfefc2ba5f57d8e025784f7fbc705dd5",
        "split": "49"
      },
      {
        "name": "BoostAfterBoost",
        "type": "node",
        "address": "03ecb3ee55ba6324d40bea174de096dc9134cb35d990235723b37ae9b5c49f4f53",
        "split": "1"
      },
      {
        "name": "MaryKateUltra",
        "address": "0325ab94e785e40877fe7421ec9a523bbb6021663dfb5c18987f40e17d5d507921",
        "type": "node",
        "customKey": "",
        "customValue": "",
        "split": "50"
      }
    ]
  },
  "type": "chapter",
  "eventGuid": "1873a383-d918-44e1-b6ff-c3598189dab6",
  "eventAPI": "https://curiohoster.com/api/sk/event/lookup",
  "blockGuid": "bcef5cc4-f534-4832-aa89-d9a26f5a0ae6",
  "startTime": 1133.701,
  "eventTimestamp": 1778198673942,
  "duration": 0
}
```

## Example 2: music item

Observed at 2026-05-15T00:30:16Z.

The polling response contained the namespace connect ack followed by two identical `remoteValue` packets in the same response body, separated by the Engine.IO record separator.

```text
40/event,{"sid":"<namespace-sid>"}
42/event,["remoteValue", { ...music payload... }]
42/event,["remoteValue", { ...same music payload again... }]
```

Payload, with the long `description` text redacted from this research note:

```json
{
  "image": "https://music.behindthesch3m3s.com/wp-content/uploads/Empath%20Eyes/East%20To%20West/Empath%20Eyes%20East%20To%20West.jpg",
  "title": "Momma Bear",
  "line": [
    "East To West",
    "Empath Eyes"
  ],
  "description": "[long text redacted in this note]",
  "value": {
    "model": {
      "type": "lightning",
      "method": "keysend"
    },
    "destinations": [
      {
        "type": "node",
        "name": "DuhLaurien",
        "address": "0368fed1c2fc35393228091c3ed1558090bfefc2ba5f57d8e025784f7fbc705dd5",
        "split": "0.49"
      },
      {
        "name": "BoostAfterBoost",
        "type": "node",
        "address": "03ecb3ee55ba6324d40bea174de096dc9134cb35d990235723b37ae9b5c49f4f53",
        "split": "0.01"
      },
      {
        "name": "MaryKateUltra",
        "address": "0325ab94e785e40877fe7421ec9a523bbb6021663dfb5c18987f40e17d5d507921",
        "type": "node",
        "customKey": "",
        "customValue": "",
        "split": "0.5"
      },
      {
        "name": "Podcastindex.org",
        "address": "03ae9f91a0cb8ff43840e3c322c4c61f019d8c1c3cea15a25cfc425ac605e61a4a",
        "type": "node",
        "fee": true,
        "split": "1"
      },
      {
        "name": "StevenB",
        "type": "node",
        "address": "035ad2c954e264004986da2d9499e1732e5175e1dcef2453c921c6cdcc3536e9d8",
        "split": "0.99"
      },
      {
        "name": "empatheyes@fountain.fm",
        "type": "node",
        "address": "03b6f613e88bd874177c28c6ad83b3baba43c4c656f56be1f8df84669556054b79",
        "split": "96.03",
        "customKey": "906608",
        "customValue": "01zBEoIV6zBgaRkqknPWKL"
      },
      {
        "name": "ThunderRoad",
        "type": "node",
        "address": "03589f3ddb81f3802f3fc9aaa359b684ed19840b55db88f7c9c2cc671e74ac93e2",
        "split": "0.99"
      },
      {
        "name": "BoostBot",
        "type": "node",
        "address": "03d55f4d4c870577e98ac56605a54c5ed20c8897e41197a068fd61bdb580efaa67",
        "split": "0.99"
      }
    ]
  },
  "type": "music",
  "link": {
    "text": "Checkout the Album",
    "url": "https://empatheyesstudio.bandcamp.com/album/east-to-west"
  },
  "chaptersUrl": null,
  "enclosureUrl": "https://music.behindthesch3m3s.com/wp-content/uploads/Empath%20Eyes/East%20To%20West/Empath%20Eyes%20-%20Momma%20Bear.mp3",
  "feedGuid": "66162162-76b3-54c2-b519-096ccd887a6f",
  "feedUrl": "https://music.behindthesch3m3s.com/wp-content/uploads/Empath Eyes/East To West/east to west.xml",
  "medium": "music",
  "itemGuid": "b7eb390a-23ec-476d-b590-e1d16e21e1bd",
  "eventGuid": "1873a383-d918-44e1-b6ff-c3598189dab6",
  "eventAPI": "https://curiohoster.com/api/sk/event/lookup",
  "duration": 248,
  "blockGuid": "be1bee54-fd00-47a2-a002-27c685e5a11f",
  "startTime": 0
}
```

## Example 3: chapter/chat prompt

Observed at 2026-05-15T00:33:29Z.

The polling response contained multiple `remoteValue` packets. One packet was a music payload with a long song-lyric `description`, which is not copied verbatim here. The response also contained this chapter packet, copied verbatim:

```text
42/event,["remoteValue",{"image":"https://feed.homegrownhits.xyz/assets/images/gif/mh1.gif","title":"In the Hitter!","line":["Hop in the chat","Pick your nick and say ITH!"],"link":{"text":"#HomegrownHits","url":"https://kiwiirc.com/nextclient/irc.zeronode.net/#homegrownhits"},"description":"","value":{"type":"lightning","method":"keysend","destinations":[{"type":"node","name":"DuhLaurien","address":"0368fed1c2fc35393228091c3ed1558090bfefc2ba5f57d8e025784f7fbc705dd5","split":"49"},{"name":"BoostAfterBoost","type":"node","address":"03ecb3ee55ba6324d40bea174de096dc9134cb35d990235723b37ae9b5c49f4f53","split":"1"},{"name":"MaryKateUltra","address":"0325ab94e785e40877fe7421ec9a523bbb6021663dfb5c18987f40e17d5d507921","type":"node","customKey":"","customValue":"","split":"50"}]},"type":"chapter","eventGuid":"1873a383-d918-44e1-b6ff-c3598189dab6","eventAPI":"https://curiohoster.com/api/sk/event/lookup","blockGuid":"7399bada-c74a-4997-9047-7d565d359f85","startTime":1723.151,"eventTimestamp":1778803481404,"duration":316800,"broadcastTimestamp":1778805204564}]
```

Formatted payload:

```json
{
  "image": "https://feed.homegrownhits.xyz/assets/images/gif/mh1.gif",
  "title": "In the Hitter!",
  "line": [
    "Hop in the chat",
    "Pick your nick and say ITH!"
  ],
  "link": {
    "text": "#HomegrownHits",
    "url": "https://kiwiirc.com/nextclient/irc.zeronode.net/#homegrownhits"
  },
  "description": "",
  "value": {
    "type": "lightning",
    "method": "keysend",
    "destinations": [
      {
        "type": "node",
        "name": "DuhLaurien",
        "address": "0368fed1c2fc35393228091c3ed1558090bfefc2ba5f57d8e025784f7fbc705dd5",
        "split": "49"
      },
      {
        "name": "BoostAfterBoost",
        "type": "node",
        "address": "03ecb3ee55ba6324d40bea174de096dc9134cb35d990235723b37ae9b5c49f4f53",
        "split": "1"
      },
      {
        "name": "MaryKateUltra",
        "address": "0325ab94e785e40877fe7421ec9a523bbb6021663dfb5c18987f40e17d5d507921",
        "type": "node",
        "customKey": "",
        "customValue": "",
        "split": "50"
      }
    ]
  },
  "type": "chapter",
  "eventGuid": "1873a383-d918-44e1-b6ff-c3598189dab6",
  "eventAPI": "https://curiohoster.com/api/sk/event/lookup",
  "blockGuid": "7399bada-c74a-4997-9047-7d565d359f85",
  "startTime": 1723.151,
  "eventTimestamp": 1778803481404,
  "duration": 316800,
  "broadcastTimestamp": 1778805204564
}
```

## Compatibility observations

- Podcast RSS advertises `/event?event_id=...`, but Engine.IO requests are made to `/socket.io/`; the Socket.IO namespace is `/event`.
- The `remoteValue` event payload is the JSON object itself, not an envelope with `event_id`, `metadata`, or `seq`.
- `type` varies by payload. Observed values include `chapter` and `music`.
- `value` shape is not fully stable:
  - Example 1 uses `value.type` and `value.method`.
  - Example 2 uses `value.model.type` and `value.model.method`.
- `value.destinations[].split` is a string, not a number. It may be integer-like (`"49"`) or decimal-like (`"96.03"`).
- Destination entries may include optional fields such as `customKey`, `customValue`, and boolean `fee`.
- Music payloads may include `chaptersUrl: null`, `enclosureUrl`, `feedGuid`, `feedUrl`, `medium`, and `itemGuid`.
- Timing fields can be numeric. Observed fields include `startTime`, `duration`, `eventTimestamp`, and `broadcastTimestamp`.
- A polling response can contain multiple Engine.IO packets separated by the record separator, including more than one `remoteValue` packet.
