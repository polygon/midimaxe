-- Ableton Link Wireshark protocol dissector (in progress; please contribute/correct)
discover = Proto("ableton_link_disc","Ableton Link Discover")
link = Proto("ableton_link","Ableton Link")

-- create a function to dissect it
function discover.dissector(buffer,pinfo,tree)
    local header = buffer(0, 7):string()
    if header ~= "_asdp_v"
    then
        return 0
    end

    pinfo.cols.protocol = "LNK/DISC"
    local subtree = tree:add(discover,buffer(),"Ableton Link Discover Protocol")
    subtree:add(buffer(7,1),"Protocol version: " .. buffer(7,1):uint())

    local message_types = {
        [0] = "Invalid",
        [1] = "Alive",
        [2] = "Response",
        [3] = "ByeBye"
    }

    local message_type = buffer(8, 1):uint();
    local mt = message_types[message_type]
    if mt
    then
        subtree:add(buffer(8, 1), "Message Type: " .. buffer(8, 1):uint() .. " [" .. mt .. "]")
    else
        subtree.add(buffer(8, 1), "Message Type: " .. buffer(8, 1):uint() .. " [Unknown Message Type]")
        return 8
    end    

    subtree:add(buffer(9,1),"TTL: " .. buffer(9,1):uint())
    subtree:add(buffer(10, 2), "Group ID: " .. buffer(10, 2):uint())
    subtree:add(buffer(12,8),"Client ID: " .. buffer(12,8):string())

    parse_payload_entries(buffer(20), subtree)

    return buffer:len()
end

function link.dissector(buffer,pinfo,tree)
    local header = buffer(0, 7):string()
    if header ~= "_link_v"
    then
        return 0
    end

    if buffer:len() < 9
    then
        return 0
    end

    pinfo.cols.protocol = "LNK"
    local subtree = tree:add(link,buffer(),"Ableton Link Protocol")
    subtree:add(buffer(0, 7),"Magic: " .. buffer(0, 7):string())
    subtree:add(buffer(7, 1),"Protoco Version: " .. buffer(7, 1):uint())

    local message_types = {
        [1] = "Ping",
        [2] = "Pong"
    }

    local message_type = buffer(8, 1):uint();
    local mt = message_types[message_type]
    if mt
    then
        subtree:add(buffer(8, 1), "Message Type: " .. buffer(8, 1):uint() .. " [" .. mt .. "]")
    else
        subtree.add(buffer(8, 1), "Message Type: " .. buffer(8, 1):uint() .. " [Unknown Message Type]")
        return 8
    end

    parse_payload_entries(buffer(9), subtree)

    return buffer:len()
end

function parse_payload_entries(buffer, tree)
    -- Known payload keys, expected lengths and decoder functions
    -- Expected length is assumed to have been checked
    local payloads = {
        ["sess"] = {
            name = "Session ID",
            len = 8,
            cnv = function(buffer, tree, k, v)
                local value = buffer(8, 8):string()
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. value)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 8), v.name .. ": " .. value)
            end
        },
        ["__ht"] = {
            name = "Host time (µs)",
            len = 8,
            cnv = function(buffer, tree, k, v)
                local value = buffer(8, 8):uint64()
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. value)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 8), v.name .. ": " .. value)
                local seconds = math.floor(value:tonumber() / 1000000)
                local s = math.fmod(seconds, 60)
                local m = math.fmod(math.floor(seconds / 60), 60)
                local h = math.floor(seconds / 3600)
                local us = math.fmod(value:tonumber(), 1000000)
                entry:add(buffer(8, 8), "[Time: " .. h .. ":" .. m .. ":" .. s .. "." .. us .. "]")
            end
        },
        ["__gt"] = {
            name = "Ghost time (µs)",
            len = 8,
            cnv = function(buffer, tree, k, v)
                local value = buffer(8, 8):uint64()
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. value)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 8), v.name .. ": " .. value)
                local seconds = math.floor(value:tonumber() / 1000000)
                local s = math.fmod(seconds, 60)
                local m = math.fmod(math.floor(seconds / 60), 60)
                local h = math.floor(seconds / 3600)
                local us = math.fmod(value:tonumber(), 1000000)
                entry:add(buffer(8, 8), "[Time: " .. h .. ":" .. m .. ":" .. s .. "." .. us .. "]")
            end
        },
        ["_pgt"] = {
            name = "Previous Ghost time (µs)",
            len = 8,
            cnv = function(buffer, tree, k, v)
                local value = buffer(8, 8):uint64()
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. value)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 8), v.name .. ": " .. value)
                local seconds = math.floor(value:tonumber() / 1000000)
                local s = math.fmod(seconds, 60)
                local m = math.fmod(math.floor(seconds / 60), 60)
                local h = math.floor(seconds / 3600)
                local us = math.fmod(value:tonumber(), 1000000)
                entry:add(buffer(8, 8), "[Time: " .. h .. ":" .. m .. ":" .. s .. "." .. us .. "]")
            end
        },
        ["tmln"] = {
            name = "Timeline",
            len = 24,
            cnv = function(buffer, tree, k, v)
                local tempo = buffer(8, 8):uint64():tonumber()
                local beatOrigin = buffer(16, 8):uint64():tonumber()
                local timeOrigin = buffer(24, 8):uint64():tonumber()
                local entry = tree:add(buffer(0, 8+v.len), string.format("Parameter [%s]: (%i, %i, %i)", v.name, tempo, beatOrigin, timeOrigin))
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 24), string.format("%s: (%i, %i, %i)", v.name, tempo, beatOrigin, timeOrigin))
                entry:add(buffer(8, 8), "Tempo (µs/beat): " .. tempo)
                entry:add(buffer(16, 8), "Beat Origin (µbeats): " .. beatOrigin)
                entry:add(buffer(24, 8), "Time Origin (µs): " .. timeOrigin)
                entry:add(buffer(8, 8), string.format("[Tempo (BPM): %.1f]", 60000000 / tempo))
                entry:add(buffer(16, 8), string.format("[Beat Origin (beats): %.2f]", beatOrigin / 1000000))
                entry:add(buffer(16, 8), string.format("[Time Origin (s): %.2f]", timeOrigin / 1000000))
            end
        },
        ["stst"] = {
            name = "Start/Stop",
            len = 17,
            cnv = function(buffer, tree, k, v)
                local isPlaying = buffer(8, 1):uint()
                local beats = buffer(9, 8):uint64():tonumber()
                local timestamp = buffer(17, 8):uint64():tonumber()
                local entry = tree:add(buffer(0, 8+v.len), string.format("Parameter [%s]: (%i, %i, %i)", v.name, isPlaying, beats, timestamp))
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 17), string.format("%s: (%i, %i, %i)", v.name, isPlaying, beats, timestamp))
                entry:add(buffer(8, 1), string.format("Is playing: %i", isPlaying))
                entry:add(buffer(9, 8), string.format("Beats (µbeats): %i", beats))
                entry:add(buffer(17, 8), string.format("Timestamp (µs): %i", timestamp))
                entry:add(buffer(8, 1), string.format("[Is playing: %s]", ((isPlaying ~= 0) and "Yes" or "No")))
                entry:add(buffer(9, 8), string.format("[Beats: %.2f]", beats / 1000000))
                entry:add(buffer(17, 8), string.format("[Timestamp (s): %.2f]", timestamp / 1000000))
            end
        },
        ["mep4"] = {
            name = "Measurement Endpoint IPv4",
            len = 6,
            cnv = function(buffer, tree, k, v)
                local address = tostring(buffer(8, 4):ipv4())
                local port = buffer(12, 2):uint()
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. address .. ":" .. port)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 4), "IPv4: " .. address)
                entry:add(buffer(12, 2), "Port: " .. port)
            end
        },
        ["mep6"] = {
            name = "Measurement Endpoint IPv6",
            len = 20,
            cnv = function(buffer, tree, k, v)
                local address = tostring(buffer(8, 16):ipv6())
                local port = buffer(24, 2):uint()
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. address .. ":" .. port)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, 16), "IPv6: " .. address)
                entry:add(buffer(24, 2), "Port: " .. port)
            end
        },
    }    

    -- Parse payload entries
    local remain = buffer
    while remain:len() > 0
    do
        key = remain(0, 4):string()
        len = remain(4, 4):uint()
        data = remain(8, len)

        converter = payloads[key]
        if not converter
        then
            converter = {
                name = "UNKNOWN",
                len = len,
                cnv = function(buffer, tree, k, v)
                    local value = buffer(8, v.len)
                    local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. value)
                    entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                    entry:add(buffer(4, 4), "Length: " .. v.len)
                    entry:add(buffer(8, v.len), v.name .. ": " .. value)
                end
            }
        end

        if len ~= converter.len
        then
            converter.name = converter.name .. " - UNEXPECTED LENGTH"
            converter.len = len
            converter.cnv = function(buffer, tree, k, v)
                local value = buffer(8, v.len)
                local entry = tree:add(buffer(0, 8+v.len), "Parameter [" .. v.name .. "]: " .. value)
                entry:add(buffer(0, 4), "Key: " .. k .. " [" .. v.name .. "]")
                entry:add(buffer(4, 4), "Length: " .. v.len)
                entry:add(buffer(8, v.len), "Data: " .. ": " .. value)
            end
        end
        
        converter.cnv(remain, tree, key, converter)

        if 8+converter.len == remain:len()
        then
            break
        end
        remain = remain(8+converter.len)
    end    
end


-- load the udp.port table
udp_table = DissectorTable.get("udp.port")
-- register our protocol to handle udp port 20808
udp_table:add(20808,discover)
-- register heuristics since both messages can also come on unpredictable ports (without knowledge of past messages)
link:register_heuristic("udp", link.dissector)
discover:register_heuristic("udp", discover.dissector)
