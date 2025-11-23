//  todo_terminal.swift
//  Terminal Todo Application
//
//  Created by Temel Gunaydin
//  Date: 2025

import Foundation

// 11 .swift_todos.json dosyasini home klasorunde tutuyoruz.
let fileURL: URL = {
    let home = FileManager.default.homeDirectoryForCurrentUser
    return home.appendingPathComponent(".swift_todos.json")
}()

// 10 bir dosyada bulunan yani json dosyasindaki veriyi Data formatina ceviriyoruz ceviremez isek bos liste döndürür.Eger basarili ise JSONDecoder().decode([String].self, from: data) ile veriyi [String] arrayine ceviriyoruz ve program icinde kullanilabilir hale getiriyoruz.
func loadTodos() -> [String] {
    guard let data = try? Data(contentsOf: fileURL) else { return [] }
    return (try? JSONDecoder().decode([String].self, from: data)) ?? []
}

// 9 String array olarak aldigimiz verii JSONEncoder ile uzerinde islem yapabilir hale getiriyoruz ve json formatina ceviriyoruz. JSONEncoder().encode datayi json  formatina cevirir ve hata firlatabilir. ? ile gelen hata verdigimiz veri json formatina cevrilemediyse nil döner. nil degil ise urettigimiz Data formatini diskte bir dosyaya yazariz.
func saveTodos(_ todos: [String]) {
    if let data = try? JSONEncoder().encode(todos) {
        try? data.write(to: fileURL)
    }
}

// 1 CommandLine.arguments komut satirindan gelen argumanlari bir [String] array olarak alir.
// todo read books -> ["todo", "read", "books"] seklinde alinir.
// dropFirst() ise ilk elemani siler ve bize ArraySlice<String> döndürür. Array() ile ise bunu arraye donusturuyoruz.
let args = Array(CommandLine.arguments.dropFirst())
// Örn: todo yarin kitap alinacak
// Örn: todo list

// 2 Eger arguman dizisi bos ise else kismi ekrana veriliyor ve program sonlanir. Eger bos degil ise loadTodo() ile todo listesini okuyoruz.
guard !args.isEmpty else {
    print("Usage:")
    print("  todo list")
    print("  todo [your todo without [] 🙂 ]")
    print("  todo del [todo number]")
    print("  todo update <todo number> <new todo>")
    exit(0)
}

// 3 loadTodos() ile todo listesini okuyoruz.
var todos = loadTodos()

// 4 Eger eger 0.arguman(cunku todos kaldirildi) list ise bu sefer todo listesi bos mu dulu mu kontrolu yapilir, bos ise ekrana mesaj verilir, dolu ise index numarasi ve kaydedilen todo ekrana yazilir.
if args[0] == "list" {
    if todos.isEmpty {
        print("No todos at all🎉")
    } else {
        // 5 Liste elemanlari ekrana renkli yaziyoruz.
        // ANSI renk kodları
        let reset = "\u{001B}[0m" // Renk sıfırlama
        let pink = "\u{001B}[35m"
        let yellow = "\u{001B}[33m" // Sarı (emoji için)

        for (index, item) in todos.enumerated() {
            print("\(pink)\(index + 1).\(reset) \(yellow)\(item)\(reset)")
        }
    }

} else if args[0] == "del" {
    // Todo silme işlemi
    guard args.count > 1 else {
        let red = "\u{001B}[31m"
        let reset = "\u{001B}[0m"
        print("\(red)Error:\(reset) Please provide a todo number to delete.")
        print("Usage: todo del <number>")
        exit(1)
    }
    
    guard let index = Int(args[1]), index > 0, index <= todos.count else {
        let red = "\u{001B}[31m"
        let reset = "\u{001B}[0m"
        print("\(red)Error:\(reset) Invalid todo number. Please use a number between 1 and \(todos.count).")
        exit(1)
    }
    
    let deletedTodo = todos.remove(at: index - 1) // index 1-based, array 0-based
    saveTodos(todos)
    
    let red = "\u{001B}[31m"
    let reset = "\u{001B}[0m"
    print("\(red)✗ Deleted:\(reset) \(deletedTodo)")

} else if args[0] == "update" {
    // Todo güncelleme işlemi
    guard args.count > 2 else {
        let red = "\u{001B}[31m"
        let reset = "\u{001B}[0m"
        print("\(red)Error:\(reset) Please provide a todo number and new text.")
        print("Usage: todo update <number> <new text>")
        exit(1)
    }
    
    guard let index = Int(args[1]), index > 0, index <= todos.count else {
        let red = "\u{001B}[31m"
        let reset = "\u{001B}[0m"
        print("\(red)Error:\(reset) Invalid todo number. Please use a number between 1 and \(todos.count).")
        exit(1)
    }
    
    let oldTodo = todos[index - 1] // index 1-based, array 0-based
    let newText = Array(args.dropFirst(2)).joined(separator: " ") // İlk 2 argümanı atla (update ve numara)
    todos[index - 1] = newText
    saveTodos(todos)
    
    let blue = "\u{001B}[34m"
    let reset = "\u{001B}[0m"
    print("\(blue)↻ Updated:\(reset)")
    print("  Old: \(oldTodo)")
    print("  New: \(newText)")

} else {
    // 5 eger list degil ise args icindeki string degerleri " " ile birlestirilir ve bir string olarak text icinde tutulur.
    let text = args.joined(separator: " ")

    // 6 append ile todo listesine yeni todo olarak eklenir.
    todos.append(text)

    // 7 saveTodos(todos) ile todo listesini diske yaziyoruz. Burdaki todos bir [String] arraydir.
    saveTodos(todos)

    // 8 ekrana eklenen todo mesajini yazdiriyoruz.
    let green = "\u{001B}[32m" // Yeşil (başarı mesajı için)
    let reset = "\u{001B}[0m" // Renk sıfırlama
    print("\(green)✓ Added:\(reset) \(text)")
}
